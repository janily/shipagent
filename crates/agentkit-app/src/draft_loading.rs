use super::*;
impl WorkbenchController {
    pub(super) fn draft_needs_load(
        &self,
        id: &str,
        store: &Entity<SessionStore>,
        cx: &App,
    ) -> bool {
        self.draft_sync.lanes.get(id).is_some_and(|lane| {
            !lane.loaded
                || (store.read(cx).model.draft.text.contains("[skill:")
                    && self.workspace_store.read(cx).skills_directory
                        != self.mention_directory(store, cx))
        })
    }
    pub(super) fn load_draft(
        &mut self,
        store: Entity<SessionStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = store.read(cx).model.state.session.id.clone();
        if self.draft_sync.lanes.get(&id).is_none_or(|l| l.loading)
            || !self.draft_needs_load(&id, &store, cx)
        {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let before = stamp(&store, cx);
        let directory = self.mention_directory(&store, cx);
        let workspace = store.read(cx).model.state.session.workspace_id.clone();
        let local_text = store.read(cx).model.draft.text.clone();
        let stored = self.draft_sync.lanes.get(&id).is_some_and(|l| !l.loaded);
        let request = if let Some(lane) = self.draft_sync.lanes.get_mut(&id) {
            lane.loading = true;
            lane.load_sequence = lane.load_sequence.wrapping_add(1);
            lane.load_sequence
        } else {
            return;
        };
        store.update(cx, |s, cx| {
            s.draft_sync.loading = true;
            s.draft_sync.error = None;
            cx.notify();
        });
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    let draft = if stored {
                        api.draft(&id).await.map_err(|e| e.to_string())?
                    } else {
                        None
                    };
                    let text = draft
                        .as_ref()
                        .map(|d| d.text.as_str())
                        .unwrap_or(&local_text);
                    let skills = if text.contains("[skill:") {
                        Some(
                            api.session_skills(&workspace, directory.as_deref())
                                .await
                                .map_err(|e| e.to_string())?,
                        )
                    } else {
                        None
                    };
                    let content = if stored {
                        let draft = draft.unwrap_or(g::SessionDraft {
                            text: String::new(),
                            attachments: None,
                        });
                        let (files, failed) = cx
                            .background_executor()
                            .spawn(async move {
                                crate::draft_io::restore(draft.attachments.unwrap_or_default())
                            })
                            .await;
                        Some((draft.text, files, failed))
                    } else {
                        None
                    };
                    Ok::<_, String>((content, skills))
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.draft_sync.lanes.get(&id).is_none_or(|l| {
                        l.stamp.entity != store.entity_id() || l.load_sequence != request
                    }) {
                        return;
                    }
                    let current = this.settings_operation_current(epoch, transport, cx)
                        && this
                            .stores
                            .get(&id)
                            .is_some_and(|s| s.entity_id() == store.entity_id());
                    let active = this
                        .state
                        .as_ref()
                        .is_some_and(|s| s.entity_id() == store.entity_id());
                    let unchanged = stamp(&store, cx) == before;
                    if let Some(lane) = this.draft_sync.lanes.get_mut(&id) {
                        lane.loading = false;
                    }
                    store.update(cx, |s, cx| {
                        s.draft_sync.loading = false;
                        cx.notify();
                    });
                    if !current || !unchanged {
                        return;
                    }
                    if active && this.mention_directory(&store, cx) != directory {
                        this.load_draft(store, window, cx);
                        return;
                    }
                    match result {
                        Ok((content, skills)) => {
                            if active && let Some(skills) = skills {
                                this.workspace_store.update(cx, |s, cx| {
                                    s.skills = skills;
                                    s.skills_directory = directory;
                                    cx.notify();
                                });
                            }
                            if let Some((text, files, failed)) = content {
                                store.update(cx, |s, cx| {
                                    s.model.set_draft(text);
                                    s.attachments.files = files;
                                    s.draft_sync.unavailable = failed;
                                    s.draft_sync.error = None;
                                    cx.notify();
                                });
                            }
                            if let Some(lane) = this.draft_sync.lanes.get_mut(&id) {
                                lane.loaded = true;
                                lane.stamp = stamp(&store, cx);
                            }
                            if active {
                                let text = store.read(cx).model.draft.text.clone();
                                this.view
                                    .update(cx, |v, cx| v.apply_draft(&id, &text, window, cx));
                            }
                        }
                        Err(error) => store.update(cx, |s, cx| {
                            s.draft_sync.error = Some(error);
                            cx.notify();
                        }),
                    }
                });
            }));
    }
    pub(in crate::controller) fn retry_draft(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.stores.get(&id).cloned() else {
            return;
        };
        if store.read(cx).draft_sync.loading {
            return;
        }
        if !store.read(cx).draft_sync.unavailable.is_empty() {
            let refs = store.read(cx).draft_sync.unavailable.clone();
            let epoch = self.epoch;
            let transport = self.transport_epoch;
            let request = if let Some(lane) = self.draft_sync.lanes.get_mut(&id) {
                lane.load_sequence = lane.load_sequence.wrapping_add(1);
                lane.loading = true;
                lane.load_sequence
            } else {
                return;
            };
            store.update(cx, |s, cx| {
                s.draft_sync.loading = true;
                cx.notify();
            });
            self.operations
                .push(cx.spawn_in(window, async move |weak, cx| {
                    let names = refs.iter().map(|r| r.reference.as_ref().clone()).collect();
                    let (files, failed) = cx
                        .background_executor()
                        .spawn(async move { crate::draft_io::restore(names) })
                        .await;
                    let _ = weak.update_in(cx, |this, _, cx| {
                        if this
                            .stores
                            .get(&id)
                            .is_none_or(|s| s.entity_id() != store.entity_id())
                            || this
                                .draft_sync
                                .lanes
                                .get(&id)
                                .is_none_or(|l| l.load_sequence != request)
                        {
                            return;
                        }
                        if let Some(lane) = this.draft_sync.lanes.get_mut(&id) {
                            lane.loading = false;
                        }
                        let current = this.settings_operation_current(epoch, transport, cx);
                        store.update(cx, |s, cx| {
                            s.draft_sync.loading = false;
                            if current
                                && s.draft_sync
                                    .unavailable
                                    .iter()
                                    .map(|r| &r.id)
                                    .eq(refs.iter().map(|r| &r.id))
                            {
                                s.attachments.files.extend(files);
                                s.draft_sync.unavailable = failed;
                            }
                            cx.notify();
                        });
                    });
                }));
        } else if self.draft_needs_load(&id, &store, cx) {
            self.load_draft(store, window, cx);
        } else {
            self.draft_writer.retry(&id);
            store.update(cx, |s, cx| {
                s.draft_sync.saving = true;
                s.draft_sync.error = None;
                cx.notify();
            });
        }
    }
}
