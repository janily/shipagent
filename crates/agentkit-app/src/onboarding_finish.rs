use super::*;
impl WorkbenchController {
    pub(super) fn finish_onboarding(
        &mut self,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_pending = true;
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = api.connections_generated().await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.settings_pending = false;
                    match result {
                        Err(e) => this.onboarding_status(false, Some(e.to_string()), cx),
                        Ok(connections) => {
                            let valid =
                                policy::completed(this.onboarding_saved.as_ref(), &connections);
                            this.app_state.update(cx, |a, cx| {
                                a.connections = connections;
                                cx.notify();
                            });
                            if !valid {
                                this.onboarding_saved = None;
                                if let Some(v) = this.view.read(cx).onboarding() {
                                    v.update(cx, |v, cx| {
                                        v.methods(cx);
                                        v.status(
                                            false,
                                            Some(agentkit_ui::t!("ak.settings.stale_editor")),
                                            cx,
                                        );
                                    });
                                }
                                return;
                            }
                            this.clear_onboarding_editor(window, cx);
                            this.view.update(cx, |v, cx| v.close_onboarding(window, cx));
                            // Reuse bootstrap's initial empty session; don't create a duplicate.
                            if this.state.as_ref().is_some_and(|s| {
                                !s.read(cx).model.state.session.messages.is_empty()
                            }) {
                                this.handle(WorkbenchIntent::NewSession, window, cx);
                            }
                        }
                    }
                });
            }));
    }
}
