//! Pure view models ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts.
//! Generated Message remains the input contract; GPUI Entity routing belongs to T-1-08.
use agentkit_protocol::generated as g;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
mod activity;
mod format;
mod grouping;
mod model;
pub use activity::*;
pub use format::*;
pub use grouping::*;
pub use model::*;

/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (deriveTurnPhase).
pub fn derive_turn_phase(turn: &AssistantTurn) -> TurnPhase {
    if turn.is_complete {
        TurnPhase::Complete
    } else if turn.response.as_ref().is_some_and(|r| r.is_streaming) {
        TurnPhase::Streaming
    } else if turn
        .activities
        .iter()
        .any(|a| a.kind == ActivityType::Tool && a.status == ActivityStatus::Running)
    {
        TurnPhase::ToolActive
    } else if !turn.activities.is_empty() {
        TurnPhase::Awaiting
    } else {
        TurnPhase::Pending
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (shouldShowThinkingIndicator).
pub fn should_show_thinking_indicator(phase: TurnPhase, buffering: bool) -> bool {
    matches!(phase, TurnPhase::Pending | TurnPhase::Awaiting)
        || (phase == TurnPhase::Streaming && buffering)
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getAssistantTurnUiKey).
pub fn get_assistant_turn_ui_key(turn: &AssistantTurn, index: usize) -> String {
    match turn
        .response
        .as_ref()
        .and_then(|r| nonempty(r.message_id.as_deref()))
    {
        Some(id) => format!("assistant:msg:{id}"),
        None => format!("assistant:turn:{}:{}:{index}", turn.turn_id, turn.timestamp),
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getTurnIntent).
pub fn get_turn_intent(turn: &AssistantTurn) -> Option<&str> {
    nonempty(turn.intent.as_deref()).or_else(|| {
        turn.activities
            .iter()
            .find_map(|a| nonempty(a.intent.as_deref()))
    })
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (hasPendingActivities).
pub fn has_pending_activities(turn: &AssistantTurn) -> bool {
    turn.activities.iter().any(|a| {
        matches!(
            a.status,
            ActivityStatus::Running | ActivityStatus::Pending | ActivityStatus::Backgrounded
        )
    })
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (hasErrorActivities).
pub fn has_error_activities(turn: &AssistantTurn) -> bool {
    turn.activities
        .iter()
        .any(|a| a.status == ActivityStatus::Error)
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getActivitySummary).
pub fn activity_summary(turn: &AssistantTurn) -> String {
    let mut parts = Vec::new();
    for (status, label) in [
        (ActivityStatus::Running, "running"),
        (ActivityStatus::Completed, "completed"),
        (ActivityStatus::Error, "failed"),
    ] {
        let count = turn
            .activities
            .iter()
            .filter(|a| a.status == status)
            .count();
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    if parts.is_empty() {
        "No activities".into()
    } else {
        parts.join(", ")
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getLastAssistantTurn).
pub fn get_last_assistant_turn(turns: &[Turn]) -> Option<&AssistantTurn> {
    turns.iter().rev().find_map(|t| {
        if let Turn::Assistant(t) = t {
            Some(t)
        } else {
            None
        }
    })
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getLastUserMessageTime).
pub fn get_last_user_message_time(turns: &[Turn]) -> Option<i64> {
    turns.iter().rev().find_map(|t| {
        if let Turn::User(t) = t {
            Some(t.timestamp)
        } else {
            None
        }
    })
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (isLastTurnStreaming).
pub fn is_last_turn_streaming(turns: &[Turn]) -> bool {
    get_last_assistant_turn(turns).is_some_and(|t| t.is_streaming)
}
fn nonempty(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}
// ECMAScript WhiteSpace + LineTerminator; unlike Rust whitespace, includes BOM and excludes NEL.
fn js_space(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
