use super::*;
use gpui_kit::component::*;
pub(crate) fn render_notices(store: &SessionStore, row: Option<usize>, cx: &App) -> AnyElement {
    let mut view = v_flex()
        .id("annotation-notices")
        .gap_2()
        .max_h(px(160.))
        .overflow_y_scroll();
    for r in store
        .approval
        .receipts
        .iter()
        .filter(|r| store.annotations.row(&r.anchor) == row)
    {
        view = view.child(
            div()
                .id(SharedString::from(format!(
                    "approval-receipt:{}",
                    r.request_id
                )))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if r.allowed {
                    if r.always {
                        t!("ak.approval.always_receipt", tool = r.tool_name.as_str())
                    } else {
                        t!("ak.approval.allowed_receipt", tool = r.tool_name.as_str())
                    }
                } else {
                    t!("ak.approval.denied_receipt", tool = r.tool_name.as_str())
                })
                .test_support(),
        );
    }
    for (index, m) in store
        .approval
        .modes
        .iter()
        .filter(|m| store.annotations.row(&m.anchor) == row)
        .enumerate()
    {
        let mode = match m.mode {
            agentkit_protocol::generated::PermissionMode::Safe => t!("ak.safe"),
            agentkit_protocol::generated::PermissionMode::Ask => t!("ak.mode_ask"),
            agentkit_protocol::generated::PermissionMode::AllowAll => t!("ak.mode_execute"),
        };
        view = view.child(
            div()
                .id(("mode-notice", index))
                .role(Role::Status)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("ak.approval.mode_changed", mode = mode.as_str())),
        );
    }
    view.into_any_element()
}
