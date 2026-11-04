use super::editor_fields::*;
use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    *,
};
impl Render for ConnectionEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("connection-editor")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .gap_3()
            .on_action(cx.listener(|this, a: &ChooseProvider, window, cx| {
                if this.busy || this.existing {
                    return;
                }
                if this.provider == a.id {
                    return;
                }
                this.generation = this.generation.wrapping_add(1);
                this.provider = a.id.clone();
                this.key.update(cx, |s, cx| s.set_value("", window, cx));
                this.model_list
                    .update(cx, |s, cx| s.set_value("", window, cx));
                this.model.update(cx, |s, cx| s.set_value("", window, cx));
                this.models.clear();
                cx.emit(EditorIntent::Provider {
                    provider: a.id.clone(),
                    generation: this.generation,
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, a: &ChooseModel, window, cx| {
                if !this.busy {
                    this.model
                        .update(cx, |s, cx| s.set_value(a.id.clone(), window, cx));
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("connection-fields")
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .gap_3()
                            .child(t!("ak.settings.name"))
                            .child(
                                Input::new(&self.name)
                                    .id("connection-name")
                                    .aria_label(t!("ak.settings.name"))
                                    .disabled(self.busy),
                            )
                            .when(!self.oauth, |v| v.child(self.kind_buttons(cx)))
                            .when(self.oauth, |v| {
                                v.child(div().text_sm().child(t!("ak.settings.oauth_existing")))
                            })
                            .when(self.kind == ConnectionKind::Preset && !self.oauth, |v| {
                                v.child(t!("ak.settings.provider"))
                                    .child(self.provider_menu())
                            })
                            .when(self.kind != ConnectionKind::Preset, |v| {
                                v.child(t!("ak.settings.base_url")).child(
                                    Input::new(&self.url)
                                        .id("connection-url")
                                        .aria_label(t!("ak.settings.base_url"))
                                        .disabled(self.busy),
                                )
                            })
                            .when(self.kind != ConnectionKind::Ollama && !self.oauth, |v| {
                                v.child(t!("ak.settings.api_key"))
                                    .when(!self.replacing, |v| {
                                        v.child(
                                            h_flex()
                                                .gap_2()
                                                .child(t!("ak.settings.key_kept"))
                                                .child(
                                                    Button::new("connection-replace-key")
                                                        .label(t!("ak.settings.replace_key"))
                                                        .disabled(self.busy)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.replacing = true;
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                    })
                                    .when(self.replacing, |v| {
                                        v.child(
                                            Input::new(&self.key)
                                                .id("connection-key")
                                                .aria_label(t!("ak.settings.api_key"))
                                                .disabled(self.busy),
                                        )
                                    })
                            })
                            .child(t!("ak.settings.default_model"))
                            .child(
                                Input::new(&self.model)
                                    .id("connection-model")
                                    .aria_label(t!("ak.settings.default_model"))
                                    .disabled(self.busy),
                            )
                            .child(self.model_menu(cx))
                            .when(self.kind != ConnectionKind::Preset, |v| {
                                v.child(t!("ak.settings.model_ids")).child(
                                    Input::new(&self.model_list)
                                        .id("connection-models")
                                        .aria_label(t!("ak.settings.model_ids"))
                                        .disabled(self.busy),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("ak.settings.key_privacy")),
                            )
                            .when_some(self.error.clone(), |v, e| {
                                v.child(
                                    div()
                                        .id("connection-editor-error")
                                        .role(Role::Alert)
                                        .aria_label(e.clone())
                                        .text_color(cx.theme().danger)
                                        .child(e)
                                        .test_support(),
                                )
                            })
                            .when_some(self.notice.clone(), |v, n| {
                                v.child(
                                    div()
                                        .id("connection-editor-notice")
                                        .role(Role::Status)
                                        .aria_label(n.clone())
                                        .text_color(cx.theme().success)
                                        .child(n)
                                        .test_support(),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("connection-editor-test")
                            .label(if self.oauth {
                                t!("ak.settings.check_auth")
                            } else {
                                t!("ak.settings.test")
                            })
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(true, cx))),
                    )
                    .child(
                        Button::new("connection-editor-save")
                            .primary()
                            .label(t!("ak.settings.save"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(false, cx))),
                    ),
            )
    }
}
