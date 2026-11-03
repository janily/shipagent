use super::*;
impl WorkbenchView {
    pub(super) fn right_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.shell.panel;
        let mut tabs = h_flex()
            .gap_1()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border);
        for (tab, label, id) in [
            (PanelTab::Files, t!("ak.files"), "panel-files"),
            (PanelTab::Plan, t!("ak.plan"), "panel-plan"),
            (PanelTab::Tasks, t!("ak.tasks"), "panel-tasks"),
        ] {
            tabs = tabs.child(
                Button::new(id)
                    .ghost()
                    .label(label.clone())
                    .selected(selected == tab)
                    .accessibility_label(label.clone())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.shell.panel = tab;
                        v.persist_shell(cx);
                        cx.notify();
                    })),
            );
        }
        let mut body = v_flex()
            .id("context-panel-body")
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .overflow_y_scroll()
            .text_sm();
        if let Some(store) = &self.state {
            let state = &store.read(cx).model.state.session;
            match selected {
                PanelTab::Files => {
                    body = body
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(t!("chat.workingDirectory")),
                        )
                        .child(
                            state
                                .working_directory
                                .clone()
                                .unwrap_or_else(|| t!("ak.no_directory")),
                        );
                }
                PanelTab::Plan => {
                    body = body.child(
                        state
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.role == agentkit_protocol::generated::MessageRole::Plan)
                            .map(|m| m.content.clone())
                            .unwrap_or_else(|| t!("ak.no_plan")),
                    );
                }
                PanelTab::Tasks => {
                    let tasks: Vec<_> = state
                        .messages
                        .iter()
                        .filter(|m| m.is_background == Some(true))
                        .collect();
                    if tasks.is_empty() {
                        body = body.child(t!("ak.no_tasks"));
                    } else {
                        for m in tasks {
                            body = body.child(
                                div()
                                    .child(
                                        m.tool_display_name
                                            .clone()
                                            .or(m.tool_name.clone())
                                            .unwrap_or_else(|| t!("ak.tasks")),
                                    )
                                    .child(match m.tool_status {
                                        Some(
                                            agentkit_protocol::generated::ToolStatus::Completed,
                                        ) => t!("ak.tool_done"),
                                        Some(agentkit_protocol::generated::ToolStatus::Error) => {
                                            t!("common.failed")
                                        }
                                        _ => t!("ak.tool_running"),
                                    }),
                            );
                        }
                    }
                }
            }
        } else {
            body = body.child(t!("ak.loading"));
        }
        v_flex()
            .id("right-context")
            .role(Role::Complementary)
            .aria_label(t!("ak.context_panel"))
            .size_full()
            .bg(cx.theme().background)
            .child(tabs)
            .child(body)
            .test_support()
            .into_any_element()
    }
}
