//! Offline replay of recorded wire events against authoritative durable transcripts.
use crate::core::{reducer::Context, session::SessionModel, turns};
use agentkit_protocol::{DecodedSessionEvent, generated as g};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
/// Recorded fixture decoding, reduction or acceptance failure.
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    /// Invalid JSON or generated DTO.
    #[error("Invalid fixture JSON: {0}")]
    Decode(#[from] serde_json::Error),
    /// Invalid event semantics.
    #[error("Fixture reduction failed: {0}")]
    Reduction(#[from] crate::core::reducer::ReducerError),
    /// A fixture invariant or comparison failed.
    #[error("{0}")]
    Validation(String),
}
impl From<&str> for FixtureError {
    fn from(s: &str) -> Self {
        Self::Validation(s.into())
    }
}
impl From<String> for FixtureError {
    fn from(s: String) -> Self {
        Self::Validation(s)
    }
}
/// Required stage-one fixture catalog. Names are also safe output basenames.
pub const SCENARIOS: &[&str] = &[
    "text",
    "single-tool",
    "parallel-tools",
    "nested-agent",
    "tool-failure",
    "permission-allow",
    "permission-deny",
    "cancel",
    "append-message",
    "retry",
    "typed-error",
    "plan",
    "compaction",
    "background-shell",
];
/// Authoritative snapshots, including independently spawned child sessions.
pub type Snapshots = BTreeMap<String, g::Session>;
/// Replay raw JSONL in order; verify cache/full projection and durable messages.
/// Renderer-only tool/status/info rows and uncommitted assistant deltas are not durable.
/// Compare complete serialized durable messages after timestamp ordering; only IDs,
/// timestamps and the documented renderer/pinned presentation omissions are excluded.
pub fn replay(initial: &str, events: &str, final_state: &str) -> Result<(), FixtureError> {
    let initial: Snapshots = serde_json::from_str(initial)?;
    let final_state: Snapshots = serde_json::from_str(final_state)?;
    if initial.is_empty() || initial.keys().ne(final_state.keys()) {
        return Err("Snapshot session set mismatch".into());
    }
    let mut stores: BTreeMap<_, _> = initial
        .into_iter()
        .map(|(id, s)| (id, SessionModel::new(s)))
        .collect();
    let mut committed = BTreeSet::new();
    for store in stores.values() {
        for m in &store.state.session.messages {
            committed.insert(m.id.clone());
        }
    }
    let mut count = 0;
    for (i, line) in events.lines().enumerate() {
        let raw: Value = serde_json::from_str(line)?;
        let event: DecodedSessionEvent = serde_json::from_value(raw.clone())?;
        if event.known().is_none() {
            return Err("Unknown event in pinned recording".into());
        }
        if raw["type"] == "text_complete"
            && let Some(id) = raw["messageId"].as_str()
        {
            committed.insert(id.to_string());
        }
        let id = raw["sessionId"].as_str().ok_or("Missing sessionId")?;
        let store = stores.get_mut(id).ok_or("Event has no initial snapshot")?;
        let context = Context {
            now_ms: 1_700_000_000_000 + i as i64,
            message_id: format!("fixture-local-{i}"),
        };
        store.apply(&event, &context)?;
        let full = turns::group_messages_by_turn(
            &store.state.session.messages,
            turns::GroupTurnsOptions {
                is_session_processing: Some(store.state.session.is_processing),
            },
        );
        if serde_json::to_value(full)? != serde_json::to_value(&store.turns)? {
            return Err(FixtureError::Validation(format!(
                "Cache differs at event {} ({})",
                i + 1,
                raw["type"]
            )));
        }
        count += 1;
    }
    if count == 0 {
        return Err("Empty event recording".into());
    }
    for (id, store) in stores {
        let expected = projection(&final_state[&id].messages, None)?;
        let actual = projection(&store.state.session.messages, Some(&committed))?;
        if actual != expected {
            return Err(FixtureError::Validation(format!(
                "Durable transcript differs for {id}: {}",
                difference(&actual, &expected)
            )));
        }
    }
    Ok(())
}
fn difference(actual: &Value, expected: &Value) -> String {
    let (Some(a), Some(b)) = (actual.as_array(), expected.as_array()) else {
        return "shape".into();
    };
    if a.len() != b.len() {
        return format!("message count {} vs {}", a.len(), b.len());
    }
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        if a != b {
            let keys: BTreeSet<_> = a
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys())
                .chain(b.as_object().into_iter().flat_map(|m| m.keys()))
                .collect();
            return format!(
                "message {i}, fields {:?}",
                keys.into_iter()
                    .filter(|k| a[*k] != b[*k])
                    .collect::<Vec<_>>()
            );
        }
    }
    "value".into()
}
fn projection(
    messages: &[g::Message],
    committed: Option<&BTreeSet<String>>,
) -> Result<Value, FixtureError> {
    let mut ordered: Vec<_> = messages.iter().collect();
    ordered.sort_by_key(|m| m.timestamp);
    let mut rows = Vec::new();
    for m in ordered {
        if let Some(ids) = committed
            && (!matches!(
                m.role,
                g::MessageRole::User
                    | g::MessageRole::Assistant
                    | g::MessageRole::Plan
                    | g::MessageRole::Error
            ) || (m.role == g::MessageRole::Assistant && !ids.contains(&m.id)))
        {
            continue;
        }
        let mut value = serde_json::to_value(m)?;
        if let Some(fields) = value.as_object_mut() {
            // Only identity/time and documented renderer-only flags are omitted.
            // The pinned server omits isIntermediate on text_complete events.
            for field in [
                "id",
                "timestamp",
                "isPending",
                "isStreaming",
                "isQueued",
                "isIntermediate",
                "errorActions",
            ] {
                fields.remove(field);
            }
        }
        rows.push(value);
    }
    Ok(Value::Array(rows))
}

/// Verify a recording exercised its named scenario, rather than merely reaching idle.
/// Returns compact, reproducible event/session evidence for the provenance manifest.
pub fn verify_scenario(
    name: &str,
    initial: &str,
    events: &str,
    final_state: &str,
) -> Result<Value, FixtureError> {
    if !SCENARIOS.contains(&name) {
        return Err("Unknown scenario".into());
    }
    let starts: Snapshots = serde_json::from_str(initial)?;
    let finals: Snapshots = serde_json::from_str(final_state)?;
    let events: Vec<Value> = events
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let has = |tag: &str| events.iter().any(|e| e["type"] == tag);
    let tools = events.iter().filter(|e| e["type"] == "tool_start").count();
    let error_tool = events
        .iter()
        .any(|e| e["type"] == "tool_result" && e["isError"] == true);
    let mut active = BTreeSet::new();
    let mut max_active = 0;
    for e in &events {
        if e["type"] == "tool_start" {
            active.insert((e["sessionId"].to_string(), e["toolUseId"].to_string()));
            max_active = max_active.max(active.len());
        } else if e["type"] == "tool_result" {
            active.remove(&(e["sessionId"].to_string(), e["toolUseId"].to_string()));
        }
    }
    let valid = match name {
        "text" => tools == 0 && has("text_delta") && has("text_complete"),
        "single-tool" => {
            tools == 1
                && events.iter().any(|e| {
                    e["type"] == "tool_result"
                        && e["result"]
                            .as_str()
                            .is_some_and(|s| s.contains("FIXTURE_READ"))
                })
        }
        "parallel-tools" => tools == 2 && max_active >= 2,
        "nested-agent" => {
            starts.len() == 3
                && starts.values().any(|s| {
                    s.parent_session_id
                        .as_ref()
                        .and_then(|p| starts.get(p))
                        .is_some_and(|p| {
                            p.parent_session_id
                                .as_ref()
                                .is_some_and(|gp| starts.contains_key(gp))
                        })
                })
                && events
                    .iter()
                    .filter(|e| e["type"] == "session_created")
                    .count()
                    == 2
        }
        "tool-failure" => tools == 1 && error_tool,
        "permission-allow" => has("permission_request") && !error_tool && has("tool_result"),
        "permission-deny" => has("permission_request") && error_tool,
        "cancel" => has("text_delta") && has("interrupted") && !has("text_complete"),
        "append-message" => {
            let ids: BTreeSet<_> = events
                .iter()
                .filter(|e| e["type"] == "user_message")
                .map(|e| e["message"]["id"].to_string())
                .collect();
            ids.len() == 2
                && finals
                    .values()
                    .map(|s| {
                        s.messages
                            .iter()
                            .filter(|m| m.role == g::MessageRole::User)
                            .count()
                    })
                    .sum::<usize>()
                    == 2
        }
        "retry" => {
            ["backoff", "active", "end"].iter().all(|p| {
                events
                    .iter()
                    .any(|e| e["type"] == "retry" && e["phase"] == *p)
            }) && !has("typed_error")
                && has("text_complete")
        }
        "typed-error" => {
            has("typed_error")
                && finals.values().any(|s| {
                    s.messages
                        .iter()
                        .any(|m| m.role == g::MessageRole::Error && m.error_code.is_some())
                })
        }
        "plan" => {
            has("plan_submitted")
                && finals
                    .values()
                    .any(|s| s.messages.iter().any(|m| m.role == g::MessageRole::Plan))
        }
        "compaction" => {
            events
                .iter()
                .any(|e| e["type"] == "info" && e["statusType"] == "compaction_complete")
                && !has("error")
        }
        "background-shell" => {
            has("shell_backgrounded")
                && events
                    .iter()
                    .any(|e| e["type"] == "task_completed" && e["status"] == "completed")
        }
        _ => false,
    };
    if !valid {
        return Err(FixtureError::Validation(format!(
            "Required evidence missing for {name}"
        )));
    }
    for id in starts.keys() {
        if !events.iter().any(|e| {
            e["sessionId"] == *id
                && (e["type"] == "complete" || (name == "cancel" && e["type"] == "interrupted"))
        }) {
            return Err("Missing terminal event".into());
        }
    }
    if finals.values().any(|s| s.is_processing) {
        return Err("Final snapshot still processing".into());
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &events {
        *counts
            .entry(e["type"].as_str().unwrap_or_default().into())
            .or_default() += 1;
    }
    Ok(json!({"sessions":starts.len(),"eventCounts":counts,"maxOverlappingTools":max_active}))
}
