use super::*;
use agentkit_protocol::generated as g;
use agentkit_store::core::plan_dispatch;
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
/// Native plan Markdown and explicit semantic acceptance actions.
pub struct PlanCard {
    store: Entity<SessionStore>,
    id: String,
    markdown: Entity<MarkdownView>,
    text: String,
    _subscription: Subscription,
    _markdown_subscription: Subscription,
}
impl EventEmitter<ChatIntent> for PlanCard {}
impl PlanCard {
    /// Bind a canonical plan; deferred loading is handled exclusively by the controller.
    pub fn new(store: Entity<SessionStore>, id: String, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        let load = store
            .read(cx)
            .model
            .state
            .session
            .messages
            .iter()
            .find(|m| m.id == id)
            .is_some_and(|m| plan_dispatch::path(m).is_some());
        if load {
            let id = id.clone();
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |_, cx| cx.emit(ChatIntent::LoadPlan(id)));
            });
        }
        let markdown = cx.new(|cx| MarkdownView::new("", false, cx));
        let markdown_subscription = cx.subscribe(&markdown, |_, _, event: &ChatIntent, cx| {
            cx.emit(event.clone())
        });
        Self {
            store,
            id,
            markdown,
            _markdown_subscription: markdown_subscription,
            text: String::new(),
            _subscription: subscription,
        }
    }
}
impl Render for PlanCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let session = &store.model.state.session;
        let Some(message) = session
            .messages
            .iter()
            .find(|m| m.id == self.id && m.role == g::MessageRole::Plan)
        else {
            return div().into_any_element();
        };
        let path = plan_dispatch::path(message);
        let has_path = path.is_some();
        let failed = store.plan.content.get(&self.id).is_some_and(Result::is_err);
        let text = if path.is_some() {
            store
                .plan
                .content
                .get(&self.id)
                .and_then(|r| r.as_ref().ok())
                .cloned()
                .unwrap_or_else(|| {
                    if failed {
                        t!("ak.plan.read_failed")
                    } else {
                        t!("ak.plan.loading")
                    }
                })
        } else {
            message.content.clone()
        };
        let latest = session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == g::MessageRole::Plan && m.hidden != Some(true))
            .is_some_and(|m| m.id == self.id);
        let waiting = path.is_some_and(|p| store.plan.pending_execution.as_deref() == Some(p));
        let accepted = store.plan.accepted.contains(&self.id)
            || waiting
            || plan_dispatch::accepted(session, &self.id);
        let disabled = store.model.busy()
            || store.model.has_pending_send()
            || store.selection_pending
            || store.plan.pending.is_some()
            || (has_path && !store.plan.content.get(&self.id).is_some_and(Result::is_ok));
        let awaiting = store.plan.awaiting_compaction;
        let hint = !store.model.draft.text.trim().is_empty();
        if self.text != text {
            self.text = text.clone();
            self.markdown
                .update(cx, |v, cx| v.replace(&text, false, cx));
        }
        let id = self.id.clone();
        let compact = self.id.clone();
        let retry = self.id.clone();
        GroupBox::new()
            .outline()
            .title(t!("ak.plan.title"))
            .child(
                v_flex()
                    .gap_3()
                    .w_full()
                    .child(self.markdown.clone())
                    .when(waiting, |v| {
                        v.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if awaiting {
                                    t!("ak.plan.waiting")
                                } else {
                                    t!("ak.plan.dispatch_wait")
                                }),
                        )
                    })
                    .when(failed, |v| {
                        v.child(
                            Button::new(SharedString::from(format!("plan-retry:{}", self.id)))
                                .label(t!("ak.plan.retry"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(ChatIntent::LoadPlan(retry.clone()))
                                })),
                        )
                    })
                    .when(latest, |v| {
                        v.child(if accepted {
                            div()
                                .id(SharedString::from(format!("plan-accepted:{}", self.id)))
                                .child(t!("ak.plan.accepted"))
                                .test_support()
                                .into_any_element()
                        } else {
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .flex_wrap()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "plan-accept:{}",
                                                self.id
                                            )))
                                            .primary()
                                            .label(t!("ak.plan.accept"))
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.emit(ChatIntent::AcceptPlan {
                                                    message_id: id.clone(),
                                                    compact: false,
                                                })
                                            })),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "plan-compact:{}",
                                                self.id
                                            )))
                                            .label(t!("ak.plan.compact"))
                                            .disabled(disabled || !has_path)
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.emit(ChatIntent::AcceptPlan {
                                                    message_id: compact.clone(),
                                                    compact: true,
                                                })
                                            })),
                                        ),
                                )
                                .when(hint, |v| {
                                    v.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .id(SharedString::from(format!(
                                                "plan-draft-hint:{}",
                                                self.id
                                            )))
                                            .test_support()
                                            .child(t!("ak.plan.draft_hint")),
                                    )
                                })
                                .into_any_element()
                        })
                    }),
            )
            .into_any_element()
    }
}
