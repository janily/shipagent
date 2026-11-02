//! Typed v1 RPC boundary. Only this module owns channels, arguments and call policy.
mod workbench;
pub use workbench::WorkbenchApi;
/// Secret-bearing wire types with redacted diagnostics.
#[allow(missing_docs)]
pub mod secrets;
/// Wire outcomes and validated request choices.
#[allow(missing_docs)]
pub mod types;
use crate::{RpcClient, RpcError};
use agentkit_protocol::generated as g;
use policy as p;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use types::*;
mod policy;
pub use policy::{Operation, V1_OPERATIONS};
#[allow(missing_docs)]
pub mod connections;
#[allow(missing_docs)]
pub mod drafts;
#[allow(missing_docs)]
pub mod files;
#[allow(missing_docs)]
pub mod oauth;
#[allow(missing_docs)]
pub mod service;
#[allow(missing_docs)]
pub mod sessions;
#[allow(missing_docs)]
pub mod settings;
#[allow(missing_docs)]
pub mod skills;
#[allow(missing_docs)]
pub mod workspaces;
impl RpcClient {
    /// Service health and versions.
    pub fn service(&self) -> service::Service<'_> {
        service::Service(self)
    }
    /// Workspace management and defaults.
    pub fn workspaces(&self) -> workspaces::Workspaces<'_> {
        workspaces::Workspaces(self)
    }
    /// Session operations and model selection.
    pub fn sessions(&self) -> sessions::Sessions<'_> {
        sessions::Sessions(self)
    }
    /// Persisted composer drafts.
    pub fn drafts(&self) -> drafts::Drafts<'_> {
        drafts::Drafts(self)
    }
    /// Attachment storage and file lookup.
    pub fn files(&self) -> files::Files<'_> {
        files::Files(self)
    }
    /// Skills discovery.
    pub fn skills(&self) -> skills::Skills<'_> {
        skills::Skills(self)
    }
    /// Model connections and provider discovery.
    pub fn connections(&self) -> connections::Connections<'_> {
        connections::Connections(self)
    }
    /// OAuth protocol calls; no browser is opened by these methods.
    pub fn oauth(&self) -> oauth::Oauth<'_> {
        oauth::Oauth(self)
    }
    /// Application settings and read-only permissions.
    pub fn settings(&self) -> settings::Settings<'_> {
        settings::Settings(self)
    }
}
async fn execute<T: DeserializeOwned>(
    client: &RpcClient,
    operation: Operation,
    args: Vec<Value>,
) -> Result<T, RpcError> {
    if operation.is_read_only() {
        client
            .invoke_read_timeout(operation.channel(), args, operation.timeout())
            .await
    } else {
        client
            .invoke_timeout(operation.channel(), args, operation.timeout())
            .await
    }
}
async fn action(
    client: &RpcClient,
    operation: Operation,
    args: Vec<Value>,
) -> Result<ActionResult, RpcError> {
    let result: ActionResult = execute(client, operation, args).await?;
    result.check()
}
fn optional<T: serde::Serialize>(args: &mut Vec<Value>, value: Option<T>) -> Result<(), RpcError> {
    if let Some(value) = value {
        args.push(serde_json::to_value(value)?);
    }
    Ok(())
}
#[allow(missing_docs)]
pub mod events;
pub use events::V1_EVENT_CHANNELS;
