use super::*;
impl WorkbenchView {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy(cx);
        let sidebar = v_flex()
            .id("session-sidebar")
            .role(Role::Navigation)
            .aria_label(t!("ak.sessions"))
            .w_full()
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .p_3()
            .gap_3()
            .child(
                div()
                    .h_9()
                    .flex()
                    .items_center()
                    .px_3()
                    .rounded_md()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(self.workspace.clone()),
            )
            .child(
                Button::new("new-session")
                    .primary()
                    .w_full()
                    .label(t!("ak.new"))
                    .disabled(!self.ready || self.loading)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::NewSession))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.sessions")),
            )
            .child(
                v_flex()
                    .id("session-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_1()
                    .when(self.status_key == "ak.starting", |v| {
                        v.children((0..3).map(|_| Skeleton::new().h_10()))
                    })
                    .children(
                        self.sessions
                            .as_ref()
                            .map(|s| s.read(cx).model.sessions.clone())
                            .unwrap_or_default()
                            .iter()
                            .map(|session| {
                                let id = session.id.clone();
                                let active = self
                                    .state
                                    .as_ref()
                                    .is_some_and(|s| s.read(cx).model.state.session.id == id);
                                let pending = self.sessions.as_ref().is_some_and(|list| {
                                    list.read(cx)
                                        .model
                                        .awaiting
                                        .get(&id)
                                        .is_some_and(|r| !r.is_empty())
                                });
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .when(pending && !active, |v| {
                                        v.child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "approval-dot:{id}"
                                                )))
                                                .size_2()
                                                .rounded_full()
                                                .bg(cx.theme().warning)
                                                .test_support(),
                                        )
                                    })
                                    .child(
                                        Button::new(SharedString::from(format!("session:{id}")))
                                            .ghost()
                                            .flex_1()
                                            .min_w_0()
                                            .justify_start()
                                            .label({
                                                let pending =
                                                    self.sessions.as_ref().is_some_and(|list| {
                                                        list.read(cx)
                                                            .model
                                                            .awaiting
                                                            .get(&id)
                                                            .is_some_and(|r| !r.is_empty())
                                                    });
                                                let plan =
                                                    self.sessions.as_ref().is_some_and(|list| {
                                                        list.read(cx).model.plans.contains(&id)
                                                    });
                                                let name = session
                                                    .name
                                                    .clone()
                                                    .or(session.preview.clone())
                                                    .unwrap_or_else(|| t!("ak.new"));
                                                format!(
                                                    "{}{}{}{}",
                                                    if session.is_processing { "• " } else { "" },
                                                    if pending {
                                                        format!("{} · ", t!("ak.permission_wait"))
                                                    } else {
                                                        String::new()
                                                    },
                                                    if plan {
                                                        format!("{} · ", t!("ak.plan"))
                                                    } else {
                                                        String::new()
                                                    },
                                                    name
                                                )
                                            })
                                            .disabled(!self.ready || self.loading)
                                            .when(active, |button| {
                                                button.bg(cx.theme().sidebar_accent)
                                            })
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.emit(WorkbenchIntent::OpenSession(id.clone()))
                                            })),
                                    )
                            }),
                    ),
            )
            .child(
                Button::new("model-settings")
                    .ghost()
                    .label(t!("ak.settings"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.setup = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("toggle-theme")
                    .ghost()
                    .label(t!("ak.theme"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::CycleAppearance))),
            );
        sidebar.test_support().into_any_element()
    }
}
