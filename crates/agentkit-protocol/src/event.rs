use crate::generated;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Validated generated event plus its lossless wire payload.
/// Known malformed events fail decoding; future event types retain their payload.
#[derive(Clone)]
pub struct DecodedSessionEvent {
    known: Option<generated::SessionEvent>,
    raw: Value,
}
impl DecodedSessionEvent {
    /// Generated event, or None for an upstream event type not yet generated.
    pub fn known(&self) -> Option<&generated::SessionEvent> {
        self.known.as_ref()
    }
    /// Complete wire payload including any future object fields.
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
impl<'de> Deserialize<'de> for DecodedSessionEvent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let kind = raw
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("Session event requires a string type"))?;
        raw.get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("Session event requires a string sessionId"))?;
        let known = if generated::SESSION_EVENT_TYPES.contains(&kind) {
            Some(serde_json::from_value(raw.clone()).map_err(serde::de::Error::custom)?)
        } else {
            None
        };
        Ok(Self { known, raw })
    }
}
impl Serialize for DecodedSessionEvent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}
impl std::fmt::Debug for DecodedSessionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedSessionEvent")
            .field("type", &self.raw.get("type"))
            .finish_non_exhaustive()
    }
}
