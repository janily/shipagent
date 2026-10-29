use crate::{WorkbenchIntent, WorkbenchView, tr};
use agentkit_protocol::prototype::Message;
use agentkit_store::RunState;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Textarea,
    message_scroller::MessageScroller,
    text::{TextView, TextViewState},
    *,
};
use gpui_kit::{prelude::*, *};
use std::collections::HashMap;

impl WorkbenchView {
    pub(crate) fn chat_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let messages = self
            .state
            .as_ref()
            .map(|state| state.session.messages.clone())
            .unwrap_or_default();
        let markdown: HashMap<String, Entity<TextViewState>> = self
            .markdown
            .iter()
            .map(|(id, (_, state))| (id.clone(), state.clone()))
            .collect();
        let expanded = self.expanded.clone();
        let weak = cx.weak_entity();
        let history = if messages.is_empty() {
            v_flex()
                .flex_1()
                .justify_center()
                .items_center()
                .gap_3()
                .child(div().text_xl().font_semibold().child(tr("ak.empty_title")))
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(if self.loading {
                            "ak.loading"
                        } else {
                            "ak.empty_hint"
                        })),
                )
                .into_any_element()
        } else {
            MessageScroller::new("messages", self.scroll.clone(), move |index, _, cx| {
                let Some(message) = messages.get(index) else {
                    return div().into_any_element();
                };
                let body = match message.role.as_str() {
                    "user" => h_flex()
                        .w_full()
                        .justify_end()
                        .child(
                            div()
                                .max_w(relative(0.8))
                                .px_4()
                                .py_3()
                                .rounded_lg()
                                .bg(cx.theme().muted)
                                .when(message.is_pending, |view| view.opacity(0.6))
                                .child(message.content.clone()),
                        )
                        .into_any_element(),
                    "assistant" => div()
                        .w_full()
                        .when_some(markdown.get(&message.id), |view, state| {
                            view.child(
                                TextView::new(state)
                                    .selectable(true)
                                    .stream_fade(message.is_streaming),
                            )
                        })
                        .into_any_element(),
                    "tool" => {
                        let id = message.id.clone();
                        let weak = weak.clone();
                        let open = expanded.contains(&id);
                        v_flex()
                            .w_full()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .when(message.parent_tool_use_id.is_some(), |view| view.ml_4())
                            .child(
                                Button::new(SharedString::from(format!("tool:{id}")))
                                    .ghost()
                                    .w_full()
                                    .justify_start()
                                    .label(tool_title(message))
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            if !this.expanded.remove(&id) {
                                                this.expanded.insert(id.clone());
                                            }
                                            this.scroll
                                                .update(cx, |scroll, cx| scroll.remeasure(cx));
                                            cx.notify();
                                        });
                                    }),
                            )
                            .when(open, |view| {
                                view.child(
                                    v_flex()
                                        .p_3()
                                        .gap_2()
                                        .text_xs()
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(tr("ak.input")),
                                        )
                                        .child(
                                            div()
                                                .font_family(cx.theme().mono_font_family.clone())
                                                .child(
                                                    message
                                                        .tool_input
                                                        .as_ref()
                                                        .map(|v| v.to_string())
                                                        .unwrap_or_default(),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(tr("ak.output")),
                                        )
                                        .child(
                                            div()
                                                .font_family(cx.theme().mono_font_family.clone())
                                                .child(
                                                    message
                                                        .tool_result
                                                        .as_deref()
                                                        .unwrap_or_default()
                                                        .chars()
                                                        .take(2000)
                                                        .collect::<String>(),
                                                ),
                                        )
                                        .when(
                                            message
                                                .tool_result
                                                .as_ref()
                                                .is_some_and(|r| r.chars().count() > 2000),
                                            |view| view.child(tr("ak.truncated")),
                                        ),
                                )
                            })
                            .into_any_element()
                    }
                    _ => div()
                        .text_color(cx.theme().muted_foreground)
                        .child(message.content.clone())
                        .into_any_element(),
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
                            .aria_label(if message.role == "tool" {
                                tool_title(message)
                            } else {
                                message.content.clone()
                            })
                            .w_full()
                            .max_w(px(820.))
                            .child(body)
                            .test_support(),
                    )
                    .into_any_element()
            })
            .with_jump_button_label(tr("ak.latest"))
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        let busy = self.busy();
        v_flex()
            .flex_1()
            .min_h_0()
            .child(history)
            .when_some(
                self.state.as_ref().and_then(|s| s.permission.clone()),
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
                                .child(div().font_semibold().child(tr("ak.permission")))
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
                                                .label(tr("ak.allow"))
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
                                        .when(permission.can_always_allow, |view| {
                                            view.child(
                                                Button::new("permission-always")
                                                    .outline()
                                                    .label(tr("ak.always"))
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
                                                .label(tr("ak.deny"))
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
                                        .aria_label(tr("ak.composer"))
                                        .appearance(false)
                                        .bordered(false)
                                        .disabled(
                                            !self.ready || self.loading || self.model.is_none(),
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
                                                .child(tr("ak.safe")),
                                        )
                                        .child(
                                            self.model.clone().unwrap_or_else(|| tr("ak.no_model")),
                                        )
                                        .when(busy, |view| {
                                            view.child(tr(
                                                if self.state.as_ref().is_some_and(|s| {
                                                    s.status == RunState::WaitingForPermission
                                                }) {
                                                    "ak.permission_wait"
                                                } else {
                                                    "ak.processing"
                                                },
                                            ))
                                        }),
                                )
                                .child(if busy {
                                    Button::new("stop")
                                        .label(tr("ak.stop"))
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.emit(WorkbenchIntent::Cancel)
                                        }))
                                        .into_any_element()
                                } else {
                                    Button::new("send")
                                        .primary()
                                        .label(tr("ak.send"))
                                        .disabled(
                                            !self.ready
                                                || self.model.is_none()
                                                || self.loading
                                                || self.submitting
                                                || self.composer.read(cx).value().trim().is_empty(),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                                        .into_any_element()
                                }),
                        ),
                ),
            )
            .into_any_element()
    }
}
fn tool_title(message: &Message) -> String {
    let (mark, key) = match message.tool_status.as_deref() {
        Some("completed") => ("✓", "ak.tool_done"),
        Some("error") => ("×", "ak.tool_error"),
        Some("interrupted") => ("—", "ak.tool_interrupted"),
        _ => ("…", "ak.tool_running"),
    };
    let hint = message
        .tool_input
        .as_ref()
        .and_then(|input| input.get("path").or_else(|| input.get("command")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    format!(
        "{mark}  {}  {}  · {}",
        message.tool_name.as_deref().unwrap_or_default(),
        hint.chars().take(100).collect::<String>(),
        tr(key)
    )
}
