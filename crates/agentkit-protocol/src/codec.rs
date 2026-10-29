//! Recursive Uint8Array wrapper codec from mkagent transport/codec.ts.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;
/// JSON-compatible value with native binary leaves.
#[derive(Clone, PartialEq)]
pub enum RpcValue {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number.
    Number(serde_json::Number),
    /// JSON string.
    String(String),
    /// Array with recursively decoded values.
    Array(Vec<RpcValue>),
    /// Object with recursively decoded values.
    Object(BTreeMap<String, RpcValue>),
    /// Upstream Uint8Array.
    Bytes(Vec<u8>),
}
impl std::fmt::Debug for RpcValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RpcValue { contents: [REDACTED] }")
    }
}
impl RpcValue {
    fn from_json(v: Value) -> Result<Self, String> {
        Ok(match v {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(v),
            Value::Number(v) => Self::Number(v),
            Value::String(v) => Self::String(v),
            Value::Array(v) => Self::Array(
                v.into_iter()
                    .map(Self::from_json)
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(mut v) => {
                if v.get("__craftRpcType").and_then(Value::as_str) == Some("u8") {
                    let data = v
                        .remove("base64")
                        .and_then(|x| x.as_str().map(str::to_owned))
                        .ok_or_else(|| "Binary wrapper requires a base64 string".to_owned())?;
                    Self::Bytes(
                        STANDARD
                            .decode(data)
                            .map_err(|_| "Invalid base64 binary wrapper".to_owned())?,
                    )
                } else {
                    Self::Object(
                        v.into_iter()
                            .map(|(k, v)| Ok((k, Self::from_json(v)?)))
                            .collect::<Result<_, String>>()?,
                    )
                }
            }
        })
    }
    fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(v) => Value::Bool(*v),
            Self::Number(v) => Value::Number(v.clone()),
            Self::String(v) => Value::String(v.clone()),
            Self::Array(v) => Value::Array(v.iter().map(Self::to_json).collect()),
            Self::Object(v) => {
                Value::Object(v.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
            Self::Bytes(v) => {
                serde_json::json!({"__craftRpcType":"u8","base64":STANDARD.encode(v)})
            }
        }
    }
}
impl Serialize for RpcValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}
impl<'de> Deserialize<'de> for RpcValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_json(Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
