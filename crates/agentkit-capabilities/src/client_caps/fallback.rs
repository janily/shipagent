use super::*;
use agentkit_protocol::RpcValue;
pub(super) fn expressible(spec: &ConfirmSpec) -> bool {
    spec.default_index() == 0
        && spec.cancel_index() == spec.buttons.len() - 1
        && spec.buttons.len() <= 3
        && spec
            .buttons
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            == spec.buttons.len()
}
pub(super) async fn confirm(spec: &ConfirmSpec) -> Result<RpcValue, WireError> {
    if spec.default_index() != 0 {
        return Err(unavailable(
            "Native fallback cannot honor a nonzero defaultId",
        ));
    }
    use rfd::{MessageButtons as B, MessageDialogResult as R, MessageLevel};
    let buttons = match spec.buttons.as_slice() {
        [a] => B::OkCustom(a.clone()),
        [a, b] => B::OkCancelCustom(a.clone(), b.clone()),
        [a, b, c] => B::YesNoCancelCustom(a.clone(), b.clone(), c.clone()),
        _ => {
            return Err(unavailable(
                "Native fallback supports at most three confirmation buttons",
            ));
        }
    };
    if spec
        .buttons
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != spec.buttons.len()
    {
        return Err(unavailable(
            "Native fallback requires distinct button labels",
        ));
    }
    let text = match &spec.detail {
        Some(detail) => format!("{}\n\n{}", spec.message, detail),
        None => spec.message.clone(),
    };
    let level = match spec.kind.as_deref() {
        Some("error") => MessageLevel::Error,
        Some("warning") => MessageLevel::Warning,
        _ => MessageLevel::Info,
    };
    let answer = rfd::AsyncMessageDialog::new()
        .set_title(&spec.title)
        .set_description(text)
        .set_level(level)
        .set_buttons(buttons)
        .show()
        .await;
    let response = match answer {
        R::Custom(label) => spec
            .buttons
            .iter()
            .position(|b| b == &label)
            .unwrap_or(spec.cancel_index()),
        R::Ok | R::Yes => 0,
        R::No => 1,
        R::Cancel => spec.cancel_index(),
    };
    serde_json::from_value(serde_json::json!({"response":response}))
        .map_err(|_| failure("Invalid confirmation result"))
}
