//! Typed v1 RPC boundary. Only this module owns channels, arguments and call policy.
mod workbench;
pub use workbench::WorkbenchApi;
/// Secret-bearing wire types with redacted diagnostics.
#[allow(missing_docs)]
pub mod secrets;
/// Wire outcomes and validated request choices.
#[allow(missing_docs)]
pub mod types;
