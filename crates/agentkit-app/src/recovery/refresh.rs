use super::*;
use agentkit_protocol::generated as g;
struct Snapshot {
    session: Option<g::Session>,
    sessions: Vec<g::Session>,
    workspaces: Vec<g::Workspace>,
    connections: Vec<g::LlmConnectionWithStatus>,
    settings: Option<g::WorkspaceSettings>,
    skills: Vec<g::LoadedSkill>,
    unread: f64,
}
async fn fetch(api: &WorkbenchApi, workspace: &str, id: Option<&str>) -> Result<Snapshot> {
    let session = if let Some(id) = id {
        api.session_if_present(id).await?
    } else {
        None
    };
    let sessions = api.sessions_generated().await?;
    let workspaces = api.workspaces_generated().await?;
    let connections = api.connections_generated().await?;
    let settings = api.workspace_settings(workspace).await?;
    let skills = api.workspace_skills(workspace).await?;
    let unread = api.unread_snapshot().await?.total_unread_sessions;
    Ok(Snapshot {
        session,
        sessions,
        workspaces,
        connections,
        settings,
        skills,
        unread,
    })
}
impl WorkbenchController {
    pub(in crate::controller) fn refresh_connection(
        &mut self,
        api: WorkbenchApi,
        snapshot: agentkit_client::ConnectionSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok((bound, _)) = api.bind_connection() else {
            return;
        };
        let Some(workspace) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        // A complete disconnect/reconnect can occur between monitor ticks.
        // Drain the previous transaction even when no offline state was observed.
        self.interrupt_refresh(window, cx);
        self.save_draft(cx);
        self.epoch = self.epoch.wrapping_add(1);
        let epoch = self.epoch;
        self.loading = false;
        self.navigation_pending = false;
        self.settings_pending = false;
        self.workspace_switching = false;
        self.list_refresh.invalidate();
        self.recovery.revision = self.recovery.revision.wrapping_add(1);
        let revision = self.recovery.revision;
        self.recovery.requested = Some(snapshot.recovery_id);
        self.recovery.pending = true;
        self.recovery.events.clear();
        self.recovery.overflow = false;
        self.close_recovery_editor(window, cx);
        for store in self.stores.values() {
            store.update(cx, |s, cx| {
                s.plan.pending = None;
                cx.notify();
            });
        }
        self.set_phase(Phase::Refreshing, cx);
        let id = self
            .state
            .as_ref()
            .map(|s| s.read(cx).model.state.session.id.clone());
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = fetch(&bound, &workspace, id.as_deref()).await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch
                        || this.recovery.revision != revision
                        || !this.recovery.pending
                        || api.snapshot().recovery_id != snapshot.recovery_id
                        || api.state() != ConnectionState::Connected
                    {
                        return;
                    }
                    this.recovery.pending = false;
                    let Ok(mut loaded) = result else {
                        let events = std::mem::take(&mut this.recovery.events);
                        this.route(events, window, cx);
                        this.recovery.force_refresh |= this.recovery.overflow;
                        this.set_phase(Phase::RefreshFailed, cx);
                        return;
                    };
                    if this.recovery.overflow {
                        let events = std::mem::take(&mut this.recovery.events);
                        this.route(events, window, cx);
                        this.recovery.force_refresh = true;
                        this.set_phase(Phase::RefreshFailed, cx);
                        return;
                    }
                    let Some(workspace_info) = loaded
                        .workspaces
                        .iter()
                        .find(|w| w.id == workspace)
                        .cloned()
                    else {
                        this.set_phase(
                            Phase::Fatal {
                                message: agentkit_ui::t!("ak.recovery.workspace_removed"),
                            },
                            cx,
                        );
                        return;
                    };
                    this.save_draft(cx);
                    for (id, store) in &this.stores {
                        this.drafts.insert(
                            id.clone(),
                            (
                                store.read(cx).model.draft.clone(),
                                store.read(cx).model.draft_attachments.clone(),
                                store.read(cx).attachments.clone(),
                            ),
                        );
                    }
                    let old = this.state.clone();
                    let rebuild = snapshot.refresh_epoch > this.transport_epoch;
                    if rebuild {
                        this.stores.clear();
                    } else {
                        this.stores
                            .retain(|id, _| loaded.sessions.iter().any(|s| &s.id == id));
                    }
                    this.workspace_sessions.clear();
                    let events = std::mem::take(&mut this.recovery.events);
                    if let Some(session) = loaded.session.take() {
                        let id = session.id.clone();
                        let store = if let Some(old) =
                            old.filter(|s| s.read(cx).model.state.session.id == id)
                        {
                            old.update(cx, |s, cx| {
                                s.selection_pending = false;
                                s.approval.pending = None;
                                s.approval.error = None;
                                if snapshot.refresh_epoch > this.transport_epoch
                                    || this.recovery.force_refresh
                                {
                                    s.refresh_snapshot(session, cx);
                                }
                            });
                            old
                        } else {
                            let mut store = SessionStore::new(session);
                            if let Some((draft, attachments, mut files)) = this.drafts.remove(&id) {
                                store.model.draft = draft;
                                store.model.draft_attachments = attachments;
                                files.reading = 0;
                                files.sending = false;
                                store.attachments = files;
                            }
                            cx.new(|_| store)
                        };

                        this.stores.insert(id.clone(), store.clone());
                        this.state = Some(store.clone());
                        this.view
                            .update(cx, |v, cx| v.bind_session(store, window, cx));
                    } else {
                        this.state = None;
                        this.view.update(cx, |v, cx| v.clear_session(cx));
                    }
                    this.list.update(cx, |s, cx| {
                        s.replace(loaded.sessions, cx);
                    });
                    this.workspace = Some(workspace_info.clone());
                    this.workspace_store.update(cx, |s, cx| {
                        s.catalog = loaded.workspaces;
                        s.name = workspace_info.name.clone();
                        s.workspace = Some(workspace_info.clone());
                        s.settings = loaded.settings;
                        s.skills = loaded.skills;
                        s.skills_directory = None;
                        cx.notify();
                    });
                    let info = api.snapshot();
                    this.transport_epoch = snapshot.recovery_id;
                    this.recovery.force_refresh = false;
                    this.app_state.update(cx, |s, cx| {
                        s.connections = loaded.connections;
                        s.unread = loaded.unread;
                        s.server_version = info.server_version;
                        s.registered_channels = info.registered_channels.unwrap_or_default();
                        cx.notify();
                    });
                    this.set_phase(Phase::Online, cx);
                    this.view
                        .update(cx, |v, cx| v.workspace_changed(workspace_info.name, cx));
                    this.remember_session(cx);
                    this.mark_viewed(window, cx);
                    this.handle_settings(agentkit_ui::settings::SettingsIntent::Reload, window, cx);
                    let mut replay = if let Some(store) = &this.state {
                        filter_snapshot_events(&store.read(cx).model.state.session, &events)
                    } else {
                        Vec::new()
                    };
                    let current = this
                        .state
                        .as_ref()
                        .map(|s| s.read(cx).model.state.session.id.clone());
                    replay.extend(events.into_iter().filter(|(event, _)| {
                        event
                            .raw()
                            .get("sessionId")
                            .and_then(serde_json::Value::as_str)
                            != current.as_deref()
                    }));
                    this.route(replay, window, cx);
                });
            }));
    }
}
