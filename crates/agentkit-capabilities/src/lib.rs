//! AgentKit capabilities layer. See docs/spec/03 模块详细设计.md.
#![deny(missing_docs)]

/// Private native system notification delivery.
pub mod notifications;

/// Native reverse-RPC dispatch and validated operating-system actions.
pub mod client_caps;

/// Open a client-owned local file with the operating system association; call on a worker.
pub fn open_local_path(path: &std::path::Path) -> Result<(), String> {
    opener::open(path).map_err(|e| e.to_string())
}
