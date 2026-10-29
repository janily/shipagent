//! Based on packages/shared/src/protocol/types.ts at mkagent v0.2.6.
use serde::{Deserialize, Serialize};
/// One transport frame. Wire keys preserve upstream camelCase.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Request correlation ID, echoed by responses.
    #[serde(default)]
    pub id: String,
    /// Frame discriminator.
    #[serde(rename = "type")]
    pub kind: EnvelopeKind,
    /// RPC channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Positional arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<serde_json::Value>>,
    /// Response result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Structured remote error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::WireError>,
    /// Protocol version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    /// Bound workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Authentication token; excluded from Debug output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Server-assigned client identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Implemented reverse RPC channels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_capabilities: Option<Vec<String>>,
    /// Server-supported channels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_channels: Option<Vec<String>>,
    /// Backend version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    /// Maximum serialized frame size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_message_bytes: Option<u64>,
    /// Event sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Last processed event sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seq: Option<u64>,
    /// Previous client identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnect_client_id: Option<String>,
    /// Whether replay resumed a previous client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnected: Option<bool>,
    /// Whether replay history expired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    /// Server identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
}
impl std::fmt::Debug for Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Envelope")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}
/// Upstream transport frame kinds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    /// Client authentication.
    Handshake,
    /// Server handshake reply.
    HandshakeAck,
    /// RPC invocation.
    #[default]
    Request,
    /// Successful invocation result.
    Response,
    /// Push event.
    Event,
    /// Failed invocation result.
    Error,
    /// Reliable delivery acknowledgement.
    SequenceAck,
}
