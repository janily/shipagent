//! Transitional UI projection over the typed facade; no independent wire calls.
use crate::{ClientOptions, ConnectionState, Endpoint, RpcClient, RpcError};
use agentkit_protocol::{
    generated as g,
    prototype::{Connection, SendAck, Session, Workspace},
};
use secrecy::SecretString;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
/// Application bridge until the formal domain/reducer migration (T-1-06).
#[derive(Clone)]
pub struct WorkbenchApi {
    client: RpcClient,
}
fn project<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U, RpcError> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}
impl WorkbenchApi {
    /// Open one workspace-bound connection.
    pub fn connect(endpoint: Endpoint, workspace_id: String) -> Self {
        Self {
            client: RpcClient::connect(ClientOptions::local(
                endpoint.url,
                endpoint.token,
                workspace_id,
            )),
        }
    }
    /// Capture connection identity before deferring a user intent.
    pub fn bind_connection(&self) -> Result<(Self, crate::ConnectionSnapshot), RpcError> {
        let (client, snapshot) = self.client.bind_connection()?;
        Ok((Self { client }, snapshot))
    }
    /// Fetch validated workspaces, then project UI fields.
    pub async fn workspaces(&self) -> Result<Vec<Workspace>, RpcError> {
        project(self.client.workspaces().list().await?)
    }
    /// Fetch validated sessions, then project UI fields.
    pub async fn sessions(&self) -> Result<Vec<Session>, RpcError> {
        project(self.client.sessions().list().await?)
    }
    /// Fetch a persisted session, rejecting a missing session explicitly.
    pub async fn session(&self, id: &str) -> Result<Session, RpcError> {
        project(self.session_generated(id).await?)
    }
    /// Full generated snapshot, before any UI field projection.
    pub async fn session_generated(&self, id: &str) -> Result<g::Session, RpcError> {
        self.client
            .sessions()
            .get_messages(id)
            .await?
            .ok_or_else(|| super::types::business_error("Session no longer exists".into()))
    }
    /// Create a session in safe mode.
    pub async fn create_session(
        &self,
        workspace: &str,
        working_directory: Option<&str>,
    ) -> Result<Session, RpcError> {
        let options = g::CreateSessionOptions {
            permission_mode: Some(g::PermissionMode::Safe),
            working_directory: working_directory.map(str::to_owned),
            ..Default::default()
        };
        project(
            self.client
                .sessions()
                .create(workspace, Some(&options))
                .await?,
        )
    }
    /// Send once with stable optimistic identity and the facade's 300s deadline.
    pub async fn send(
        &self,
        session: &str,
        text: &str,
        optimistic_id: &str,
    ) -> Result<SendAck, RpcError> {
        let options = g::SendMessageOptions {
            optimistic_message_id: Some(optimistic_id.into()),
            ..Default::default()
        };
        let ack = self
            .client
            .sessions()
            .send_message(
                session,
                text,
                super::types::SendParts {
                    options: Some(&options),
                    ..Default::default()
                },
            )
            .await?;
        Ok(SendAck {
            accepted: ack.accepted,
            message_id: ack.message_id,
        })
    }
    /// Cancel once; writes are never automatically retried.
    pub async fn cancel(&self, session: &str) -> Result<(), RpcError> {
        self.client.sessions().cancel(session, None).await
    }
    /// Send an explicit user's decision and verify the backend accepted the response.
    pub async fn respond_permission(
        &self,
        session: &str,
        request: &str,
        allowed: bool,
        always: bool,
    ) -> Result<(), RpcError> {
        if self
            .client
            .sessions()
            .respond_permission(session, request, allowed, always)
            .await?
        {
            Ok(())
        } else {
            Err(super::types::business_error(
                "Backend did not accept the permission response".into(),
            ))
        }
    }
    /// Query authentication status without retrieving credentials.
    pub async fn connections(&self) -> Result<Vec<Connection>, RpcError> {
        project(self.client.connections().list_with_status().await?)
    }
    /// Configure a compatible endpoint; report both setup and default-selection failures.
    pub async fn setup_endpoint(
        &self,
        base_url: &str,
        model: &str,
        credential: Option<&SecretString>,
    ) -> Result<(), RpcError> {
        let setup = super::secrets::ConnectionSetup {
            slug: "workbench-endpoint".into(),
            credential: credential.cloned(),
            base_url: Some(Some(base_url.into())),
            default_model: Some(Some(model.into())),
            models: Some(Some(vec![model.into()])),
            custom_endpoint: Some(g::CustomEndpointConfig {
                api: g::CustomEndpointConfigApi::OpenaiCompletions,
                supports_images: Some(false),
                max_tokens: Some(2048.0),
            }),
            ..Default::default()
        };
        self.client.connections().setup(&setup).await?;
        self.client
            .connections()
            .set_default("workbench-endpoint")
            .await?;
        Ok(())
    }
    /// Keep unsupported browser tooling disabled.
    pub async fn disable_browser(&self) -> Result<(), RpcError> {
        self.client.settings().disable_browser().await
    }
    /// Raw event arguments retained for the existing batch reducer; it validates ingress.
    pub fn session_events(&self) -> async_channel::Receiver<Vec<Value>> {
        self.client
            .subscribe(agentkit_protocol::constants::channels::sessions::EVENT)
    }
    /// Current lifecycle state.
    pub fn state(&self) -> ConnectionState {
        self.client.state()
    }
    /// Atomic connection and cache-refresh metadata.
    pub fn snapshot(&self) -> crate::ConnectionSnapshot {
        self.client.snapshot()
    }
    /// Last transport error, cleared on successful recovery.
    pub fn last_error(&self) -> Option<RpcError> {
        self.client.last_error()
    }
    /// Stop this connection and all its retries.
    pub fn close(&self) {
        self.client.close();
    }
}
