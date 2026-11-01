//! Wire primitives for mkagent v0.2.6 (protocol 1.0).
#![deny(missing_docs)]
mod codec;
mod envelope;
mod event;
pub use event::DecodedSessionEvent;
mod error;
pub use codec::RpcValue;
/// Supported upstream protocol version.
pub use constants::PROTOCOL_VERSION;
pub use envelope::{Envelope, EnvelopeKind};
pub use error::{ErrorCode, WireError};

/// Credentials for a local backend; Debug never exposes the token.
#[derive(Clone, Debug)]
pub struct Endpoint {
    /// Loopback WebSocket URL.
    pub url: String,
    /// Per-process authentication secret.
    pub token: secrecy::SecretString,
}

/// Transitional UI/domain projections, with generated validation for events.
pub mod prototype;

/// Generated core wire DTOs, including reviewed integer overrides and named events.
#[allow(missing_docs, clippy::derivable_impls)]
pub mod generated;

/// Generated upstream RPC channels, version and message-size constants.
#[allow(missing_docs)]
pub mod constants;
