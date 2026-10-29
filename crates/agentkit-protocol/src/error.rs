//! Forward-compatible remote error codes.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
macro_rules! error_codes {
    ($($name:ident => $wire:literal),* $(,)?) => {
        /// Known upstream codes with a lossless unknown-code fallback.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ErrorCode {
            $(#[doc = $wire] $name,)*
            /// A code added by a newer backend.
            Other(String),
        }
        impl ErrorCode {
            /// Original wire spelling.
            pub fn as_str(&self) -> &str {
                match self { $(Self::$name => $wire,)* Self::Other(code) => code }
            }
        }
        impl<'de> Deserialize<'de> for ErrorCode {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let code = String::deserialize(d)?;
                Ok(match code.as_str() { $($wire => Self::$name,)* _ => Self::Other(code) })
            }
        }
    }
}
error_codes! {
    HandlerError => "HANDLER_ERROR", ChannelNotFound => "CHANNEL_NOT_FOUND",
    AuthFailed => "AUTH_FAILED", ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
    SessionNotIdle => "SESSION_NOT_IDLE", SessionIdConflict => "SESSION_ID_CONFLICT",
    RequestTimeout => "REQUEST_TIMEOUT", ClientRequestTimeout => "CLIENT_REQUEST_TIMEOUT",
    ClientDisconnected => "CLIENT_DISCONNECTED", CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
    TransferTooLarge => "TRANSFER_TOO_LARGE", TransferTimeout => "TRANSFER_TIMEOUT",
    TransferVerificationFailed => "TRANSFER_VERIFICATION_FAILED", ArtifactNotPortable => "ARTIFACT_NOT_PORTABLE",
    BrowserNoCapableClient => "BROWSER_NO_CAPABLE_CLIENT", BrowserInstanceNotOwned => "BROWSER_INSTANCE_NOT_OWNED",
    BrowserRemoteUploadNotSupported => "BROWSER_REMOTE_UPLOAD_NOT_SUPPORTED",
    BrowserRemoteEvaluateBlocked => "BROWSER_REMOTE_EVALUATE_BLOCKED",
}
impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
/// Structured remote failure. Debug deliberately excludes message and data.
#[derive(Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct WireError {
    /// Stable machine-readable error code.
    pub code: ErrorCode,
    /// User-readable error explanation.
    pub message: String,
    /// Optional upstream details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
impl std::fmt::Debug for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WireError")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}
