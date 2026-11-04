use crate::{WorkbenchIntent, WorkbenchView, t};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
use gpui_kit::{prelude::*, *};

impl WorkbenchView {
    pub(crate) fn chat_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let history = if self.loading {
            div().flex_1().child(t!("ak.loading")).into_any_element()
        } else {
            self.chat.clone().into_any_element()
        };
        let connections = self
            .app_state
            .as_ref()
            .map(|s| s.read(cx).connections.as_slice())
            .unwrap_or_default();
        let choices = crate::composer::model_choices(connections);
        let settings = self
            .workspace_state
            .as_ref()
            .and_then(|s| s.read(cx).settings.as_ref());
        let session = self.state.as_ref().map(|s| &s.read(cx).model.state.session);
        let (effective_model, selected_connection) = if self.app_state.is_some() {
            crate::composer::resolved_model(connections, session, settings)
        } else {
            (
                session
                    .and_then(|s| s.model.clone())
                    .or_else(|| self.model.clone()),
                None,
            )
        };
        let thinking = settings
            .and_then(|s| s.thinking_level)
            .unwrap_or(agentkit_protocol::generated::ThinkingLevel::Medium);
        self.composer.update(cx, |c, cx| {
            c.configure(
                crate::composer::Config {
                    store: self.state.clone(),
                    ready: self.ready,
                    submitting: self.submitting,
                    loading: self.loading,
                    choices,
                    model: effective_model,
                    connection: selected_connection,
                    thinking,
                },
                cx,
            )
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .child(history)
            .when_some(
                self.state
                    .as_ref()
                    .and_then(|s| s.read(cx).model.permissions.first().cloned()),
                |view, permission| {
                    let allow_id = permission.request_id.clone();
                    let deny_id = permission.request_id.clone();
                    let always_id = permission.request_id.clone();
                    view.child(
                        h_flex().justify_center().px_6().py_2().child(
                            v_flex()
                                .id("permission-card")
                                .w_full()
                                .max_w(px(820.))
                                .border_1()
                                .border_color(cx.theme().warning)
                                .bg(cx.theme().muted)
                                .rounded_lg()
                                .p_4()
                                .gap_3()
                                .child(div().font_semibold().child(t!("ak.permission")))
                                .child(permission.description)
                                .when_some(permission.command, |view, command| {
                                    view.child(
                                        div()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .child(command),
                                    )
                                })
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("permission-allow")
                                                .primary()
                                                .label(t!("ak.allow"))
                                                .disabled(!self.ready || self.permission_busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.permission_busy = true;
                                                    cx.emit(WorkbenchIntent::Permission {
                                                        request_id: allow_id.clone(),
                                                        allowed: true,
                                                        always: false,
                                                    });
                                                    cx.notify();
                                                })),
                                        )
                                        .when(permission.can_always_allow == Some(true), |view| {
                                            view.child(
                                                Button::new("permission-always")
                                                    .outline()
                                                    .label(t!("ak.always"))
                                                    .disabled(!self.ready || self.permission_busy)
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.permission_busy = true;
                                                            cx.emit(WorkbenchIntent::Permission {
                                                                request_id: always_id.clone(),
                                                                allowed: true,
                                                                always: true,
                                                            });
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                        })
                                        .child(
                                            Button::new("permission-deny")
                                                .outline()
                                                .label(t!("ak.deny"))
                                                .disabled(!self.ready || self.permission_busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.permission_busy = true;
                                                    cx.emit(WorkbenchIntent::Permission {
                                                        request_id: deny_id.clone(),
                                                        allowed: false,
                                                        always: false,
                                                    });
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        ),
                    )
                },
            )
            .child(
                h_flex()
                    .justify_center()
                    .px_6()
                    .py_4()
                    .child(self.composer.clone()),
            )
            .into_any_element()
    }
}
