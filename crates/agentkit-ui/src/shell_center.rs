use super::*;
impl WorkbenchView {
    pub(super) fn center(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let title = self
            .state
            .as_ref()
            .and_then(|s| s.read(cx).model.state.session.name.clone())
            .unwrap_or_else(|| self.app_name.to_string());
        let content = if !self.ready && self.status_key == "ak.starting" {
            v_flex()
                .id("startup-skeleton")
                .role(Role::Status)
                .aria_label(t!("ak.starting"))
                .flex_1()
                .p_6()
                .gap_4()
                .child(Skeleton::new().w_48())
                .child(Skeleton::new())
                .child(Skeleton::new().w_64())
                .test_support()
                .into_any_element()
        } else if self.setup {
            h_flex()
                .id("endpoint-scroll")
                .flex_1()
                .min_h_0()
                .justify_center()
                .overflow_y_scroll()
                .child(
                    v_flex().gap_4().p_6().child(t!("ak.onboard.empty")).child(
                        Button::new("add-model-connection")
                            .primary()
                            .label(t!("ak.settings.add"))
                            .on_click(
                                cx.listener(|v, _, window, cx| v.show_configuration(window, cx)),
                            ),
                    ),
                )
                .into_any_element()
        } else {
            self.chat_body(window, cx)
        };
        v_flex()
            .track_focus(&self.center_focus)
            .key_context("Conversation")
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                h_flex()
                    .h(px(48.))
                    .px_6()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .child(
                        div()
                            .id("session-title")
                            .font_semibold()
                            .child(title)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                                    if e.click_count == 2
                                        && this.ready
                                        && !this.loading
                                        && let Some(s) = &this.state
                                    {
                                        cx.emit(WorkbenchIntent::Sidebar(
                                            crate::sidebar::SidebarIntent::Rename(
                                                s.read(cx).model.state.session.id.clone(),
                                            ),
                                        ));
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                self.state
                                    .as_ref()
                                    .and_then(|s| {
                                        s.read(cx).model.state.session.working_directory.clone()
                                    })
                                    .unwrap_or_default(),
                            ),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    h_flex()
                        .px_6()
                        .py_2()
                        .gap_3()
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .id("session-error")
                                .role(Role::Alert)
                                .aria_label(error.clone())
                                .flex_1()
                                .text_color(cx.theme().danger)
                                .child(error)
                                .test_support(),
                        )
                        .when(!self.ready, |view| {
                            view.child(
                                Button::new("retry")
                                    .outline()
                                    .label(t!("ak.retry"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(WorkbenchIntent::Reconnect)
                                    })),
                            )
                        }),
                )
            })
            .child(content)
            .into_any_element()
    }
}
