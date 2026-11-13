use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
impl Composer {
    pub(super) fn draft_status(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut row = h_flex().id("draft-status").test_support().gap_2().text_xs();
        if let Some(store) = &self.store {
            let s = store.read(cx);
            let id = s.model.state.session.id.clone();
            let state = &s.draft_sync;
            if state.loading {
                row = row.child(t!("ak.draft.loading"));
            } else if let Some(error) = &state.error {
                let error = match error.as_str() {
                    "ak.draft.content_too_large" => t!("ak.draft.content_too_large"),
                    "ak.draft.missing_content" => t!("ak.draft.missing_content"),
                    "ak.draft.invalid_content" => t!("ak.draft.invalid_content"),
                    "ak.draft.worker_stopped" => t!("ak.draft.worker_stopped"),
                    "ak.draft.unsaved" => t!("ak.draft.unsaved"),
                    _ => error.clone(),
                };
                row = row
                    .child(
                        div()
                            .text_color(cx.theme().danger)
                            .child(t!("ak.draft.failed", error = error.as_str())),
                    )
                    .child(
                        Button::new("retry-draft")
                            .ghost()
                            .small()
                            .label(t!("ak.draft.retry"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(WorkbenchIntent::RetryDraft(id.clone()))
                            })),
                    );
            } else if !state.unavailable.is_empty() {
                row = row
                    .child(
                        div()
                            .text_color(cx.theme().warning)
                            .child(t!("ak.draft.attachments_unavailable")),
                    )
                    .child(
                        Button::new("retry-draft")
                            .ghost()
                            .small()
                            .label(t!("ak.draft.retry"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(WorkbenchIntent::RetryDraft(id.clone()))
                            })),
                    );
            } else if state.saving {
                row = row
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.draft.saving"));
            }
        }
        row
    }
}
