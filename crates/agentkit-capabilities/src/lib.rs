//! AgentKit capabilities layer. See docs/spec/03 模块详细设计.md.
#![deny(missing_docs)]

/// Private native system notification delivery.
pub mod notifications;

/// Native reverse-RPC dispatch and validated operating-system actions.
pub mod client_caps;
