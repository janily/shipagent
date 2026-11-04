use super::*;
impl WorkbenchController {
    pub(super) fn mutate_form(
        &mut self,
        id: Option<String>,
        form: Entity<NavigationDialog>,
        value: NavigationForm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.navigation_pending || self.loading {
            form.update(cx, |f, cx| {
                f.status(false, Some(agentkit_ui::t!("ak.nav.operation_busy")), cx)
            });
            return;
        }
        let Some(api) = self
            .api()
            .and_then(|a| a.bind_connection().ok().map(|a| a.0))
        else {
            form.update(cx, |f, cx| {
                f.status(
                    false,
                    Some(agentkit_ui::t!("ak.connection_unavailable")),
                    cx,
                )
            });
            return;
        };
        self.navigation_pending = true;
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result: Result<Option<Workspace>> = async {
                    if let Some(id) = &id {
                        api.rename_session(id, &value.name).await?;
                        Ok(None)
                    } else {
                        let slug = std::path::Path::new(&value.directory)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .context(agentkit_ui::t!("ak.nav.required"))?;
                        let check = api.check_workspace_slug(slug).await?;
                        anyhow::ensure!(
                            !check.exists
                                || std::path::Path::new(&check.path)
                                    == std::path::Path::new(&value.directory),
                            agentkit_ui::t!("ak.nav.slug_exists")
                        );
                        Ok(Some(
                            api.create_workspace(&value.directory, &value.name).await?,
                        ))
                    }
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.navigation_pending = false;
                    match result {
                        Ok(Some(workspace)) => {
                            this.workspace_store.update(cx, |s, cx| {
                                s.catalog.retain(|w| w.id != workspace.id);
                                s.catalog.push(workspace.clone());
                                cx.notify();
                            });
                            window.close_dialog(cx);
                            this.switch_navigation(workspace, api, window, cx);
                        }
                        Ok(None) => {
                            if let Some(id) = &id {
                                this.list.update(cx, |list, cx| {
                                    if let Some(s) =
                                        list.model.sessions.iter_mut().find(|s| s.id == *id)
                                    {
                                        s.name = Some(value.name.clone());
                                    }
                                    cx.notify();
                                });
                                if let Some(store) = this.stores.get(id) {
                                    store.update(cx, |s, cx| {
                                        s.model.state.session.name = Some(value.name);
                                        cx.notify();
                                    });
                                }
                            }
                            window.close_dialog(cx);
                        }
                        Err(e) => form.update(cx, |f, cx| f.status(false, Some(e.to_string()), cx)),
                    }
                });
            }));
    }
    pub(super) fn remove_navigation(
        &mut self,
        workspace: bool,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.navigation_pending {
            return;
        }
        if workspace && self.workspace.as_ref().is_none_or(|w| w.id == id) {
            return;
        }
        if !workspace && !self.list.read(cx).model.sessions.iter().any(|s| s.id == id) {
            return;
        }
        let Some(api) = self
            .api()
            .and_then(|a| a.bind_connection().ok().map(|a| a.0))
        else {
            return;
        };
        self.navigation_pending = true;
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = if workspace {
                    api.remove_workspace(&id).await
                } else {
                    api.delete_session(&id).await
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.navigation_pending = false;
                    match result {
                        Ok(()) => {
                            if workspace {
                                this.workspace_store.update(cx, |s, cx| {
                                    s.catalog.retain(|w| w.id != id);
                                    cx.notify();
                                });
                            } else {
                                this.list.update(cx, |s, cx| {
                                    s.model.sessions.retain(|s| s.id != id);
                                    s.model.awaiting.remove(&id);
                                    s.model.plans.remove(&id);
                                    cx.notify();
                                });
                                this.stores.remove(&id);
                                this.drafts.remove(&id);
                                if this
                                    .state
                                    .as_ref()
                                    .is_some_and(|s| s.read(cx).model.state.session.id == id)
                                {
                                    this.state = None;
                                    this.view.update(cx, |v, cx| v.clear_session(cx));
                                }
                                if this.state.is_none() {
                                    this.handle(WorkbenchIntent::NewSession, window, cx);
                                }
                            }
                        }
                        Err(e) => this
                            .view
                            .update(cx, |v, cx| v.show_error(e.to_string(), false, cx)),
                    }
                });
            }));
    }
}
