use crate::{WorkbenchIntent, WorkbenchView, t};
use agentkit_store::core::turns::{ActivityStatus, ActivityType, Turn};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Textarea,
    message_scroller::MessageScroller,
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};

impl WorkbenchView {
    pub(crate) fn chat_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.state.clone();
        let count = store.as_ref().map_or(0, |s| s.read(cx).model.turns.len());
        let weak = cx.weak_entity();
        let history = if count == 0 {
            v_flex()
                .flex_1()
                .justify_center()
                .items_center()
                .child(if self.loading {
                    t!("ak.loading")
                } else {
                    t!("ak.empty_hint")
                })
                .into_any_element()
        } else {
            MessageScroller::new("messages", self.scroll.clone(), move |index, _, cx| {
                let Some(view) = weak.upgrade() else {
                    return div().into_any_element();
                };
                let view_state = view.read(cx);
                let markdown = &view_state.markdown;
                let expanded = &view_state.expanded;
                let Some(store) = &store else {
                    return div().into_any_element();
                };
                let Some(turn) = store.read(cx).model.turns.get(index) else {
                    return div().into_any_element();
                };
                let mut row = v_flex().w_full().gap_2();
                let label = match turn {
                    Turn::User(t) => {
                        row = row.items_end().child(
                            div()
                                .px_4()
                                .py_3()
                                .rounded_lg()
                                .bg(cx.theme().muted)
                                .when(t.message.is_pending == Some(true), |v| v.opacity(0.6))
                                .child(t.message.content.clone()),
                        );
                        t.message.content.clone()
                    }
                    Turn::System(t) => {
                        row = row.child(t.message.content.clone());
                        t.message.content.clone()
                    }
                    Turn::Assistant(t) => {
                        for activity in &t.activities {
                            if activity.kind == ActivityType::Intermediate {
                                if t.response.as_ref().and_then(|r| r.message_id.as_ref())
                                    == Some(&activity.id)
                                {
                                    continue;
                                }
                                if let Some((_, state)) = markdown.get(&activity.id) {
                                    row = row.child(
                                        TextView::new(state).selectable(true).stream_fade(
                                            activity.status == ActivityStatus::Running,
                                        ),
                                    );
                                }
                            } else {
                                let id = activity.id.clone();
                                let weak = weak.clone();
                                let open = expanded.contains(&id);
                                let title = format!(
                                    "{} · {}",
                                    activity.tool_name.clone().unwrap_or_else(|| {
                                        if activity.kind == ActivityType::Plan {
                                            t!("ak.plan")
                                        } else {
                                            String::new()
                                        }
                                    }),
                                    match activity.status {
                                        ActivityStatus::Completed => t!("ak.tool_done"),
                                        ActivityStatus::Error => t!("common.failed"),
                                        _ => t!("ak.tool_running"),
                                    }
                                );
                                row = row.child(
                                    v_flex()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .rounded_md()
                                        .child(
                                            Button::new(SharedString::from(format!("tool:{id}")))
                                                .ghost()
                                                .label(title)
                                                .on_click(move |_, _, cx| {
                                                    let _ = weak.update(cx, |this, cx| {
                                                        if !this.expanded.remove(&id) {
                                                            this.expanded.insert(id.clone());
                                                        }
                                                        this.scroll
                                                            .update(cx, |s, cx| s.remeasure(cx));
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .when(open, |v| {
                                            v.child(
                                                serde_json::to_string(&activity.tool_input)
                                                    .unwrap_or_default(),
                                            )
                                            .child(
                                                activity
                                                    .content
                                                    .as_deref()
                                                    .unwrap_or_default()
                                                    .chars()
                                                    .take(2000)
                                                    .collect::<String>(),
                                            )
                                        }),
                                );
                            }
                        }
                        if let Some(response) = &t.response
                            && let Some(id) = &response.message_id
                            && let Some((_, state)) = markdown.get(id)
                        {
                            row = row.child(
                                TextView::new(state)
                                    .selectable(true)
                                    .stream_fade(response.is_streaming),
                            );
                        }
                        t.response
                            .as_ref()
                            .map(|r| r.text.clone())
                            .or_else(|| {
                                t.activities
                                    .iter()
                                    .rev()
                                    .find(|a| a.kind == ActivityType::Intermediate)
                                    .and_then(|a| a.content.clone())
                            })
                            .unwrap_or_default()
                    }
                };
                h_flex()
                    .w_full()
                    .justify_center()
                    .px_6()
                    .py_2()
                    .child(
                        div()
                            .id(("message", index))
                            .role(Role::Group)
                            .aria_label(label)
                            .w_full()
                            .max_w(px(820.))
                            .child(row)
                            .test_support(),
                    )
                    .into_any_element()
            })
            .with_jump_button_label(t!("ak.latest"))
            .flex_1()
            .min_h_0()
            .into_any_element()
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
