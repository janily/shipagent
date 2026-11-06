use super::*;
use agentkit_ui::{
    onboarding::{OnboardingIntent, resolve_slug},
    settings::{ConnectionEditor, EditorIntent},
};
#[path = "onboarding_finish.rs"]
mod finish;
#[path = "onboarding_policy.rs"]
mod policy;
#[path = "onboarding_submission.rs"]
mod submission;
impl WorkbenchController {
    fn onboarding_status(&self, busy: bool, error: Option<String>, cx: &mut Context<Self>) {
        if let Some(v) = self.view.read(cx).onboarding() {
            v.update(cx, |v, cx| v.status(busy, error, cx));
        }
    }
    fn clear_onboarding_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = self.settings_editor.take() {
            e.update(cx, |e, cx| e.clear_key(window, cx));
        }
        self.settings_editor_subscription = None;
    }
    pub(super) fn handle_onboarding(
        &mut self,
        intent: OnboardingIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(wizard) = self.view.read(cx).onboarding() else {
            return;
        };
        if self.settings_pending {
            return;
        }
        if matches!(intent, OnboardingIntent::Back) {
            self.clear_onboarding_editor(window, cx);
            self.onboarding_saved = None;
            wizard.update(cx, |v, cx| v.methods(cx));
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            self.onboarding_status(
                false,
                Some(agentkit_ui::t!("ak.connection_unavailable")),
                cx,
            );
            return;
        };
        if matches!(intent, OnboardingIntent::Finish) {
            self.finish_onboarding(api, window, cx);
            return;
        }
        self.settings_pending = true;
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = match &intent {
                    OnboardingIntent::Defer => {
                        api.defer_setup().await.map(|_| (Vec::new(), Vec::new()))
                    }
                    OnboardingIntent::Choose(_) => {
                        async {
                            let catalog = api.connections_generated().await?;
                            let providers = api.connection_providers().await?;
                            Ok((
                                catalog,
                                providers.into_iter().map(|p| (p.key, p.label)).collect(),
                            ))
                        }
                        .await
                    }
                    _ => unreachable!(),
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.settings_pending = false;
                    match result {
                        Err(e) => this.onboarding_status(false, Some(e.to_string()), cx),
                        Ok((catalog, providers)) => match intent {
                            OnboardingIntent::Defer => {
                                this.preferences.update(|p| p.onboarding_deferred = true);
                                this.clear_onboarding_editor(window, cx);
                                this.view.update(cx, |v, cx| v.close_onboarding(window, cx));
                            }
                            OnboardingIntent::Choose(kind) => {
                                this.app_state.update(cx, |a, cx| {
                                    a.connections = catalog;
                                    cx.notify();
                                });
                                this.onboarding_saved = None;
                                let slug = resolve_slug(
                                    this.app_state
                                        .read(cx)
                                        .connections
                                        .iter()
                                        .map(|c| c.slug.clone()),
                                );
                                let editor = cx.new(|cx| {
                                    ConnectionEditor::new(slug, None, providers, window, cx)
                                });
                                editor.update(cx, |e, cx| e.prepare_onboarding(kind, window, cx));
                                this.settings_editor_subscription = Some(cx.subscribe_in(
                                    &editor,
                                    window,
                                    move |this, editor, intent: &EditorIntent, window, cx| {
                                        if this.epoch == epoch {
                                            this.editor_action(
                                                editor.clone(),
                                                intent.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    },
                                ));
                                this.settings_editor = Some(editor.clone());
                                wizard.update(cx, |v, cx| v.edit(editor.clone(), cx));
                                let focus = editor.clone();
                                window.defer(cx, move |window, cx| {
                                    focus.update(cx, |e, cx| e.focus(window, cx))
                                });
                                if let Some((provider, generation)) =
                                    editor.read(cx).catalog_request()
                                {
                                    this.editor_action(
                                        editor,
                                        EditorIntent::Provider {
                                            provider,
                                            generation,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }
                            _ => {}
                        },
                    }
                });
            }));
    }
}
