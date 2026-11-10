use super::*;
use agentkit_protocol::generated as g;
use gpui_kit::component::{
    bubble::{Bubble, BubbleVariant},
    message::{Message, MessageAlignment, MessageContent},
    tag::Tag,
};
pub(super) fn render(message: g::Message, parent: WeakEntity<ChatView>) -> impl IntoElement {
    let mut content = h_flex().flex_wrap().gap_1();
    let mut offset = 0;
    for badge in message.badges.iter().flatten() {
        if let Some(range) =
            agentkit_store::core::mentions::byte_range(&message.content, badge.start, badge.end)
            && range.start >= offset
            && range.end > range.start
        {
            if range.start > offset {
                content = content.child(message.content[offset..range.start].to_owned());
            }
            content = content.child(Tag::new().child(badge.label.clone()));
            offset = range.end;
        }
    }
    if offset < message.content.len() {
        content = content.child(message.content[offset..].to_owned());
    }
    let mut body = v_flex().gap_2().child(content);
    if message.is_queued == Some(true) {
        body = body.child(Tag::new().child(t!("ak.chat.queued")));
    }
    for attachment in message.attachments.iter().flatten() {
        body = body.child(crate::attachment::card(
            attachment.name.clone(),
            attachment.thumbnail_base64.as_deref(),
        ));
    }
    let copy = message.id.clone();
    let branch = message.id.clone();
    let branch_parent = parent.clone();
    v_flex()
        .items_end()
        .gap_1()
        .child(
            Message::new().alignment(MessageAlignment::End).content(
                MessageContent::new().bubble(
                    Bubble::new()
                        .with_variant(BubbleVariant::Muted)
                        .max_w(relative(0.8))
                        .when(message.is_pending == Some(true), |v| v.opacity(0.6))
                        .when(message.is_queued == Some(true), |v| {
                            v.border_1().border_dashed()
                        })
                        .child(body),
                ),
            ),
        )
        .child(
            h_flex()
                .gap_1()
                .child(
                    Button::new(SharedString::from(format!("copy:{copy}")))
                        .ghost()
                        .label(t!("ak.chat.copy"))
                        .on_click(move |_, _, cx| {
                            emit(&parent, ChatIntent::CopyMessage(copy.clone()), cx)
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("branch:{branch}")))
                        .ghost()
                        .label(t!("ak.chat.branch"))
                        .disabled(message.is_pending == Some(true))
                        .on_click(move |_, _, cx| {
                            emit(&branch_parent, ChatIntent::BranchFrom(branch.clone()), cx)
                        }),
                ),
        )
}
