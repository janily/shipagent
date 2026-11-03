use crate::{WorkbenchIntent, WorkbenchView, t};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Textarea,
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
        let busy = self.busy(cx);
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
                h_flex().justify_center().px_6().py_4().child(
                    v_flex()
                        .w_full()
                        .max_w(px(820.))
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_lg()
                        .p_3()
                        .gap_3()
                        .child(
                            div()
                                .id("composer")
                                .w_full()
                                .child(
                                    Textarea::new(&self.composer)
                                        .aria_label(t!("ak.composer"))
                                        .appearance(false)
                                        .bordered(false)
                                        .disabled(
                                            !self.ready
                                                || self.state.is_none()
                                                || self.loading
                                                || self.model.is_none(),
                                        ),
                                )
                                .test_support(),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            div()
                                                .px_2()
                                                .py_1()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded_full()
                                                .child(match self.state.as_ref().and_then(|s|s.read(cx).model.state.session.permission_mode) {Some(agentkit_protocol::generated::PermissionMode::Ask)=>t!("ak.mode_ask"),Some(agentkit_protocol::generated::PermissionMode::AllowAll)=>t!("ak.mode_execute"),_=>t!("ak.safe")}),
                                        )
                                        .child(
                                            self.model.clone().unwrap_or_else(|| t!("ak.no_model")),
                                        )
                                        .when(busy, |view| {
                                            view.child(if self.state.as_ref().is_some_and(|s| {
                                                    !s.read(cx).model.permissions.is_empty()
                                                }) {
                                                    t!("ak.permission_wait")
                                                } else {
                                                    t!("ak.processing")
                                                })
                                        }),
                                )
                                .when(busy, |v| {
                                    v.child(Button::new("stop").label(t!("ak.stop")).on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::Cancel)),
                                    ))
                                })
                                .child(
                                    Button::new("send")
                                        .primary()
                                        .label(t!("ak.send"))
                                        .disabled(
                                            !self.ready
                                                || self.model.is_none()
                                                || self.loading
                                                || self.submitting
                                                || self.composer.read(cx).value().trim().is_empty(),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                                ),
                        ),
                ),
            )
            .into_any_element()
    }
}
