use super::*;
use agentkit_ui::composer::model_choices;
impl WorkbenchController {
    pub(super) fn change_composer_selection(
        &mut self,
        intent: WorkbenchIntent,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.state.clone() else {
            return;
        };
        if let WorkbenchIntent::ModelChanged {
            ref connection,
            ref model,
        } = intent
            && !model_choices(&self.app_state.read(cx).connections)
                .iter()
                .any(|m| &m.connection == connection && &m.id == model)
        {
            self.view.update(cx, |v, cx| {
                v.selection_finished(cx);
                v.show_error(agentkit_ui::t!("ak.compose.model_unavailable"), false, cx);
            });
            return;
        }
        if store.read(cx).plan.pending.is_some() {
            self.view.update(cx, |v, cx| v.selection_finished(cx));
            return;
        }
        if store.read(cx).selection_pending {
            return;
        }
        store.update(cx, |s, cx| {
            s.selection_pending = true;
            cx.notify();
        });
        let session = &store.read(cx).model.state.session;
        let id = session.id.clone();
        let workspace = session.workspace_id.clone();
        let epoch = self.epoch;
        self.operations.retain(|task| !task.is_ready());
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = match &intent {
                    WorkbenchIntent::ModelChanged { connection, model } => {
                        api.set_session_model(&id, &workspace, model, connection)
                            .await
                    }
                    WorkbenchIntent::ThinkingChanged(level) => {
                        api.set_session_thinking(&id, *level).await
                    }
                    WorkbenchIntent::ModeChanged(mode) => api.set_session_mode(&id, *mode).await,
                    _ => return,
                };
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this.epoch != epoch
                        || this
                            .stores
                            .get(&id)
                            .is_none_or(|s| s.entity_id() != store.entity_id())
                    {
                        return;
                    }
                    store.update(cx, |s, cx| {
                        s.selection_pending = false;
                        cx.notify();
                    });
                    if result.is_ok() {
                        store.update(cx, |s, cx| {
                            let session = &mut s.model.state.session;
                            match intent {
                                WorkbenchIntent::ModelChanged { connection, model } => {
                                    session.model = Some(model);
                                    session.llm_connection = Some(connection);
                                }
                                WorkbenchIntent::ThinkingChanged(level) => {
                                    session.thinking_level = Some(level)
                                }
                                WorkbenchIntent::ModeChanged(mode) => {
                                    session.permission_mode = Some(mode)
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                    }
                    if this
                        .state
                        .as_ref()
                        .is_some_and(|s| s.entity_id() == store.entity_id())
                    {
                        this.view.update(cx, |v, cx| {
                            v.selection_finished(cx);
                            if let Err(error) = result {
                                v.show_error(error.to_string(), false, cx);
                            }
                        });
                    }
                });
            }));
    }
}
