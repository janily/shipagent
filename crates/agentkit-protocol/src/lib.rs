//! Wire primitives for mkagent v0.2.6 (protocol 1.0).
#![deny(missing_docs)]
mod codec;
mod envelope;
mod error;
pub use codec::RpcValue;
pub use envelope::{Envelope, EnvelopeKind};
pub use error::{ErrorCode, WireError};
/// Supported upstream protocol version.
pub const PROTOCOL_VERSION: &str = "1.0";
