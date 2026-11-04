use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    kbd::Kbd,
    *,
};
impl Render for PermissionPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_focus && !window.has_active_dialog(cx) && !window.has_active_sheet(cx) {
            if self.previous.is_none() {
                self.previous = window.focused(cx);
            }
            self.focus.focus(window, cx);
            self.needs_focus = false;
        }
        let Some(store) = &self.store else {
            return div().into_any_element();
        };
        let s = store.read(cx);
        let Some(request) = s.model.permissions.first() else {
            return div().into_any_element();
        };
        let disabled = !self.ready || self.local_pending || s.approval.pending.is_some();
        v_flex()
            .id("permission-card")
            .test_support()
            .w_full()
            .max_w(px(820.))
            .max_h(px((f32::from(window.bounds().size.height) - 300.).max(180.)))
            .overflow_y_scroll()
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().muted)
            .rounded_lg()
            .p_4()
            .gap_3()
            .track_focus(&self.focus)
            .key_context("PermissionPrompt")
            .on_action(
                cx.listener(|this, _: &ApprovePermission, _, cx| this.respond(true, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &AlwaysApprovePermission, _, cx| {
                    this.respond(true, true, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &RejectPermission, _, cx| this.respond(false, false, cx)),
            )
            .capture_action(cx.listener(|this, a: &Enter, _, cx| {
                if !a.shift {
                    this.respond(true, a.secondary, cx);
                } else {
                    cx.propagate();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, _, cx| this.respond(false, false, cx)))
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Info))
                    .child(
                        div()
                            .flex_1()
                            .font_semibold()
                            .child(t!("ak.approval.title", tool = request.tool_name.as_str())),
                    )
                    .child(t!(
                        "ak.approval.queue",
                        current = 1,
                        total = s.model.permissions.len()
                    )),
            )
            .child(request.description.clone())
            .when_some(request.command.clone(), |v, text| {
                v.child(
                    div()
                        .id("permission-command")
                        .test_support()
                        .w_full()
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .rounded_md()
                        .bg(cx.theme().background)
                        .p_3()
                        .font_family(cx.theme().mono_font_family.clone())
                        .aria_label(text.clone())
                        .child(text),
                )
            })
            .when_some(request.reason.clone(), |v, text| {
                v.child(t!("ak.approval.reason", text = text.as_str()))
            })
            .when_some(request.impact.clone(), |v, text| {
                v.child(t!("ak.approval.impact", text = text.as_str()))
            })
            .when_some(
                s.approval
                    .error
                    .as_ref()
                    .filter(|(id, _)| id == &request.request_id)
                    .map(|(_, e)| e.clone()),
                |v, error| {
                    v.child(
                        div()
                            .id("permission-error")
                            .test_support()
                            .text_color(cx.theme().danger)
                            .aria_label(error.clone())
                            .child(error),
                    )
                },
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("permission-allow")
                            .primary()
                            .label(t!("ak.allow"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.respond(true, false, cx))),
                    )
                    .when_some(
                        Kbd::binding_for_action_in(&ApprovePermission, &self.focus, window),
                        |v, kbd| v.child(kbd),
                    )
                    .when(request.can_always_allow == Some(true), |v| {
                        v.child(
                            Button::new("permission-always")
                                .outline()
                                .label(t!("ak.always"))
                                .disabled(disabled)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.respond(true, true, cx)),
                                ),
                        )
                        .when_some(
                            Kbd::binding_for_action_in(
                                &AlwaysApprovePermission,
                                &self.focus,
                                window,
                            ),
                            |v, kbd| v.child(kbd),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("permission-deny")
                            .outline()
                            .label(t!("ak.deny"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| this.respond(false, false, cx))),
                    )
                    .when_some(
                        Kbd::binding_for_action_in(&RejectPermission, &self.focus, window),
                        |v, kbd| v.child(kbd),
                    ),
            )
            .into_any_element()
    }
}
