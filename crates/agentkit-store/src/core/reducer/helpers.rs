//! Ported from mkagent v0.2.6 event-processor/helpers.ts.
use super::*;
use serde::{Serialize, de::DeserializeOwned};
pub(super) fn convert<T: Serialize, U: DeserializeOwned>(value: &T) -> Result<U, ReducerError> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}
pub(super) fn time(value: Option<f64>, fallback: i64) -> Result<i64, ReducerError> {
    match value {
        None => Ok(fallback),
        Some(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= i64::MIN as f64
                && value < 9223372036854775808.0 =>
        {
            Ok(value as i64)
        }
        _ => Err(ReducerError::InvalidTimestamp),
    }
}
pub(crate) fn message(ctx: &Context, role: g::MessageRole, content: String) -> g::Message {
    g::Message {
        annotations: None,
        attachments: None,
        badges: None,
        content,
        elapsed_seconds: None,
        error_actions: None,
        error_can_retry: None,
        error_code: None,
        error_details: None,
        error_original: None,
        error_title: None,
        hidden: None,
        id: ctx.message_id.clone(),
        info_level: None,
        is_background: None,
        is_error: None,
        is_intermediate: None,
        is_pending: None,
        is_queued: None,
        is_streaming: None,
        parent_tool_use_id: None,
        plan_path: None,
        role,
        shell_id: None,
        status_type: None,
        task_id: None,
        timestamp: ctx.now_ms,
        tool_display_meta: None,
        tool_display_name: None,
        tool_duration: None,
        tool_input: None,
        tool_intent: None,
        tool_name: None,
        tool_result: None,
        tool_status: None,
        tool_use_id: None,
        turn_id: None,
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/helpers.ts.
pub(super) fn streaming(messages: &[g::Message], turn: Option<&str>) -> Option<usize> {
    turn.filter(|id| !id.is_empty())
        .and_then(|id| {
            messages.iter().position(|m| {
                m.role == g::MessageRole::Assistant
                    && m.turn_id.as_deref() == Some(id)
                    && m.is_streaming == Some(true)
            })
        })
        .or_else(|| {
            messages
                .iter()
                .rposition(|m| m.role == g::MessageRole::Assistant && m.is_streaming == Some(true))
        })
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/helpers.ts.
pub(super) fn assistant(messages: &[g::Message], turn: Option<&str>) -> Option<usize> {
    turn.filter(|id| !id.is_empty())
        .and_then(|id| {
            messages.iter().position(|m| {
                m.role == g::MessageRole::Assistant && m.turn_id.as_deref() == Some(id)
            })
        })
        .or_else(|| streaming(messages, None))
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/helpers.ts.
pub(super) fn tool_index(state: &SessionState, id: &str) -> Option<usize> {
    state
        .session
        .messages
        .iter()
        .position(|m| m.tool_use_id.as_deref() == Some(id))
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/helpers.ts.
pub(super) fn append(
    session: &mut g::Session,
    message: g::Message,
    update_time: bool,
    ctx: &Context,
) {
    if !message.id.is_empty() && session.messages.iter().any(|m| m.id == message.id) {
        return;
    }
    session.last_message_role = match message.role {
        g::MessageRole::User => Some(g::SessionLastMessageRole::User),
        g::MessageRole::Assistant => Some(g::SessionLastMessageRole::Assistant),
        g::MessageRole::Plan => Some(g::SessionLastMessageRole::Plan),
        g::MessageRole::Tool => Some(g::SessionLastMessageRole::Tool),
        g::MessageRole::Error => Some(g::SessionLastMessageRole::Error),
        _ => session.last_message_role,
    };
    session.messages.push(message);
    if update_time {
        session.last_message_at = ctx.now_ms;
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/helpers.ts.
pub(super) fn clear_retry(session: &mut g::Session) {
    session.messages.retain(|m| {
        !(m.role == g::MessageRole::Status && m.status_type == Some(g::MessageStatusType::Retrying))
    });
    if session
        .current_status
        .as_ref()
        .is_some_and(|s| s.status_type.as_deref() == Some("retrying"))
    {
        session.current_status = None;
    }
}
