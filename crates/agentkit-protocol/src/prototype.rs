//! Handwritten phase-zero DTO subset, ported from mkagent v0.2.6 protocol/dto.ts.
//! This is explicitly not the final generated protocol surface (T-0-07/T-1-01).
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Phase-zero Workspace fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Workspace {
    /// Backend workspace identity.
    pub id: String,
    /// Workspace display name.
    pub name: String,
    /// Workspace root directory.
    pub root_path: String,
}
/// Phase-zero Message fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Message {
    /// Server or optimistic message identity.
    pub id: String,
    /// Upstream message role.
    pub role: String,
    /// Message text.
    pub content: String,
    /// Associated assistant turn.
    pub turn_id: Option<String>,
    /// Tool name.
    pub tool_name: Option<String>,
    /// Tool invocation identity.
    pub tool_use_id: Option<String>,
    /// Tool arguments.
    pub tool_input: Option<Value>,
    /// Tool output.
    pub tool_result: Option<String>,
    /// Tool lifecycle state.
    pub tool_status: Option<String>,
    /// Nested tool parent.
    pub parent_tool_use_id: Option<String>,
    /// Optimistic message flag.
    pub is_pending: bool,
    /// Text is still arriving.
    pub is_streaming: bool,
    /// Tool or message failure.
    pub is_error: bool,
    /// Intermediate assistant text.
    pub is_intermediate: bool,
}
/// Phase-zero Session fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Session {
    /// Session identity.
    pub id: String,
    /// Owning workspace.
    pub workspace_id: String,
    /// Session title.
    pub name: Option<String>,
    /// Last message preview.
    pub preview: Option<String>,
    /// Persisted messages, lazily loaded.
    pub messages: Vec<Message>,
    /// Backend processing state.
    pub is_processing: bool,
    /// Effective working directory.
    pub working_directory: Option<String>,
    /// Selected model.
    pub model: Option<String>,
    /// safe, ask or allow-all.
    pub permission_mode: Option<String>,
}
/// Phase-zero PermissionRequest fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionRequest {
    /// Owning session.
    pub session_id: String,
    /// Permission decision identity.
    pub request_id: String,
    /// Requested tool.
    pub tool_name: String,
    /// User-facing explanation.
    pub description: String,
    /// Command requiring approval.
    pub command: Option<String>,
    /// Whether session-wide grant is supported.
    pub can_always_allow: bool,
}
/// Phase-zero Connection fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Connection {
    /// Connection identity.
    pub slug: String,
    /// Connection display name.
    pub name: String,
    /// Whether backend can use this connection.
    pub is_authenticated: bool,
    /// Whether it is the global default.
    pub is_default: bool,
    /// Default model identifier.
    pub default_model: Option<String>,
}
/// Phase-zero SendAck fields; unknown upstream fields remain compatible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SendAck {
    /// Message was persisted.
    pub accepted: bool,
    /// Persisted user-message identity.
    pub message_id: String,
}

/// Lossless event boundary. Specific phase-zero events are interpreted by the reducer.
#[derive(Clone)]
pub struct SessionEvent {
    /// Bound session identity, used before reduction to prevent cross-session updates.
    pub session_id: String,
    /// Original upstream event discriminator.
    pub kind: String,
    /// Complete original payload, including future event fields.
    pub payload: Value,
}
impl<'de> Deserialize<'de> for SessionEvent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let payload = Value::deserialize(d)?;
        let session_id = payload
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("Session event requires sessionId"))?
            .to_owned();
        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("Session event requires type"))?
            .to_owned();
        Ok(Self {
            session_id,
            kind,
            payload,
        })
    }
}
impl std::fmt::Debug for SessionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionEvent")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Serialize for SessionEvent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.payload.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[allow(clippy::expect_used)]
    fn future_session_event_round_trips_without_losing_fields() {
        let wire =
            serde_json::json!({"type":"future_event","sessionId":"s","unknown":{"nested":[1,2]}});
        let event: super::SessionEvent = serde_json::from_value(wire.clone()).expect("event");
        assert_eq!(serde_json::to_value(event).expect("serialize"), wire);
    }
}
