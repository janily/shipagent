use super::*;
struct WorkspaceSnapshot {
    sessions: Vec<Session>,
    session: Session,
    settings: Option<agentkit_protocol::generated::WorkspaceSettings>,
    skills: Vec<agentkit_protocol::generated::LoadedSkill>,
}
async fn load_workspace(
    api: &WorkbenchApi,
    id: &str,
    preferred: Option<&str>,
) -> Result<WorkspaceSnapshot> {
    let settings = api.workspace_settings(id).await?;
    let skills = api.workspace_skills(id).await?;
    let mut sessions = api.sessions_generated().await?;
    sessions.retain(|s| s.workspace_id == id);
    sessions.sort_by_key(|s| std::cmp::Reverse(s.last_message_at));
    let session = if let Some(s) = sessions
        .iter()
        .filter(|s| s.hidden != Some(true) && s.is_archived != Some(true))
        .min_by_key(|s| {
            if Some(s.id.as_str()) == preferred {
                0
            } else {
                1
            }
        }) {
        api.session_generated(&s.id).await?
    } else {
        let session = api
            .create_session_generated(
                id,
                settings
                    .as_ref()
                    .and_then(|s| s.working_directory.as_deref()),
            )
            .await?;
        sessions.push(session.clone());
        session
    };
    anyhow::ensure!(
        session.workspace_id == id,
        agentkit_ui::t!("ak.nav.workspace_mismatch")
    );
    Ok(WorkspaceSnapshot {
        sessions,
        session,
        settings,
        skills,
    })
}
impl WorkbenchController {
    pub(super) fn switch_navigation(
        &mut self,
        workspace: Workspace,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(old) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        if self
            .stores
            .values()
            .any(|s| s.read(cx).model.has_pending_send())
        {
            self.view.update(cx, |v, cx| {
                v.show_error(agentkit_ui::t!("ak.nav.operation_busy"), false, cx)
            });
            return;
        }
        self.save_draft(cx);
        if let Some(s) = &self.state {
            self.workspace_sessions
                .insert(old.clone(), s.read(cx).model.state.session.id.clone());
        }
        let preferred = self.workspace_sessions.get(&workspace.id).cloned();
        self.loading = true;
        self.workspace_switching = true;
        self.backlog.clear();
        self.backlog_overflow = false;
        self.list_refresh.invalidate();
        self.view.update(cx, |v, cx| v.set_loading(cx));
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    api.switch_workspace(&workspace.id).await?;
                    load_workspace(&api, &workspace.id, preferred.as_deref()).await
                }
                .await;
                let rollback = if result.is_err() {
                    api.switch_workspace(&old).await.is_ok()
                } else {
                    true
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.loading = false;
                    this.workspace_switching = false;
                    if this.backlog_overflow {
                        this.backlog.clear();
                        this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.load_overflow"), true, cx)
                        });
                        return;
                    }
                    match result {
                        Ok(snapshot) => {
                            this.workspace = Some(workspace.clone());
                            this.workspace_store.update(cx, |s, cx| {
                                s.id = workspace.id.clone();
                                s.name = workspace.name.clone();
                                s.workspace = Some(workspace.clone());
                                s.settings = snapshot.settings;
                                s.skills = snapshot.skills;
                    s.skills_directory=None;
                                cx.notify();
                            });
                            let buffered=std::mem::take(&mut this.backlog);
                            let id=snapshot.session.id.clone();
                            if buffered.iter().any(|(e,_)|matches!(e.known(),Some(agentkit_protocol::generated::SessionEvent::SessionDeletedEvent(e)) if e.session_id==id)){
                                this.view.update(cx,|v,cx|v.show_error(agentkit_ui::t!("ak.session_deleted"),true,cx));return;
                            }
                            this.list.update(cx,|s,cx|{s.replace(snapshot.sessions,cx);s.apply_batch(buffered.iter().map(|(e,_)|e),cx);});
                            let replay=filter_snapshot_events(&snapshot.session,&buffered);
                            let state=if let Some(state)=this.stores.get(&id).cloned(){
                                state.update(cx,|s,cx|{s.refresh_snapshot(snapshot.session,cx);s.apply_batch(&replay,cx);});state
                            }else{
                                let mut model=SessionStore::new(snapshot.session);
                                if let Some(draft)=this.drafts.remove(&id){draft.restore(&mut model);}
                                let state=cx.new(|_|model);state.update(cx,|s,cx|{s.apply_batch(&replay,cx);});state
                            };
                            this.stores.insert(id.clone(),state.clone());
                            this.state = Some(state.clone());
                            if let Some(runtime) = this.runtime.borrow_mut().as_mut() {
                                runtime.session_id = Some(id);
                            }
                            this.remember_session(window,cx);
                            this.view.update(cx, |v, cx| {
                                v.workspace_changed(workspace.name, cx);
                                v.bind_session(state, window, cx);
                            });
                            this.mark_viewed(window, cx);
                            this.route(Vec::new(),window,cx);
                        }
                        Err(error) => this
                            .view
                            .update(cx, |v, cx| v.show_error(error.to_string(), !rollback, cx)),
                    }
                });
            }));
    }
}
