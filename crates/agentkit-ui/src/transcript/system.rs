use super::*;
use agentkit_protocol::generated as g;
use gpui_kit::component::{
    alert::Alert,
    marker::{Marker, MarkerAlignment, MarkerContent, MarkerVariant},
};
pub(super) fn content(message: &g::Message) -> String {
    if message.role == g::MessageRole::Info
        && message.error_code.as_deref() == Some("BACKEND_INTERRUPTED")
    {
        t!("ak.recovery.interrupted")
    } else {
        message.content.clone()
    }
}
pub(super) fn render(
    message: g::Message,
    parent: WeakEntity<ChatView>,
    cache: &Rc<RefCell<Cache>>,
    _cx: &App,
) -> AnyElement {
    if message.role != g::MessageRole::Error {
        let variant = if message.role == g::MessageRole::Status {
            MarkerVariant::Separator
        } else {
            MarkerVariant::Plain
        };
        return Marker::new()
            .id(SharedString::from(format!("system:{}", message.id)))
            .role(Role::Status)
            .alignment(MarkerAlignment::Center)
            .text_sm()
            .with_variant(variant)
            .content(MarkerContent::new().text(content(&message)))
            .into_any_element();
    }
    let title = message
        .error_title
        .clone()
        .unwrap_or_else(|| t!("ak.chat.error"));
    let mut body = v_flex()
        .id(SharedString::from(format!("error:{}", message.id)))
        .role(Role::Alert)
        .aria_label(format!("{title}: {}", message.content))
        .gap_2()
        .child(Alert::error("typed-error", message.content.clone()).title(title));
    let mut actions = message.error_actions.clone().unwrap_or_default();
    if message.error_can_retry == Some(true)
        && !actions
            .iter()
            .any(|a| a.action == Some(g::MessageErrorActionsItemAction::Retry))
    {
        actions.push(g::MessageErrorActionsItem {
            key: "retry".into(),
            label: t!("ak.chat.retry"),
            action: Some(g::MessageErrorActionsItemAction::Retry),
            url: None,
        });
    }
    for action in actions {
        if action.action.is_none() {
            continue;
        }
        let id = message.id.clone();
        let key = action.key.clone();
        let parent = parent.clone();
        body = body.child(
            Button::new(SharedString::from(format!("error-action:{id}:{key}")))
                .outline()
                .label(action.label)
                .on_click(move |_, _, cx| {
                    emit(
                        &parent,
                        ChatIntent::ErrorAction {
                            message_id: id.clone(),
                            key: key.clone(),
                        },
                        cx,
                    )
                }),
        );
    }
    // Technical payloads remain selectable but collapsed until explicitly asked.
    let details = message
        .error_details
        .iter()
        .flatten()
        .cloned()
        .chain(message.error_original.clone())
        .collect::<Vec<_>>()
        .join("\n");
    if !details.is_empty() {
        let id = message.id.clone();
        let open = cache.borrow().details.contains(&id);
        body = body.child(
            gpui_kit::component::collapsible::Collapsible::new()
                .open(open)
                .child(
                    Button::new(SharedString::from(format!("error-details:{id}")))
                        .ghost()
                        .label(t!("ak.chat.technical"))
                        .on_click(move |_, _, cx| {
                            let _ = parent.update(cx, |view, cx| {
                                {
                                    let mut cache = view.cache.borrow_mut();
                                    if !cache.details.remove(&id) {
                                        cache.details.insert(id.clone());
                                    }
                                }
                                view.scroll.update(cx, |s, cx| s.remeasure(cx));
                                cx.notify();
                            });
                        }),
                )
                .content(
                    div()
                        .id("error-technical-details")
                        .text_sm()
                        .child(details)
                        .test_support(),
                ),
        );
    }
    body.test_support().into_any_element()
}
