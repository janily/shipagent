//! Native application facade and retained compatibility projections over typed RPC groups.
use crate::{ClientOptions, ConnectionState, Endpoint, RpcClient, RpcError};
use agentkit_protocol::{
    generated as g,
    prototype::{Connection, SendAck, Session, Workspace},
};
use secrecy::SecretString;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
/// Controller facade; production entity paths use the generated snapshot methods.
#[derive(Clone)]
pub struct WorkbenchApi {
    client: RpcClient,
}
fn project<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U, RpcError> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}
impl WorkbenchApi {
    /// Verify backend health before selecting the startup workspace and session.
    pub async fn health(&self) -> Result<(), RpcError> {
        let _ = self.client.service().health().await?;
        Ok(())
    }

    /// Replace the local sidecar generation; pending calls are rejected without replay.
    pub fn replace_endpoint(&self, endpoint: Endpoint) {
        self.client.replace_endpoint(endpoint);
    }
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
    /// Generated workspace context for native entities.
    pub async fn workspaces_generated(&self) -> Result<Vec<g::Workspace>, RpcError> {
        self.client.workspaces().list().await
    }
    /// Safe model connection status, without credentials.
    pub async fn connections_generated(&self) -> Result<Vec<g::LlmConnectionWithStatus>, RpcError> {
        self.client.connections().list_with_status().await
    }
    /// Workspace settings snapshot.
    pub async fn workspace_settings(
        &self,
        id: &str,
    ) -> Result<Option<g::WorkspaceSettings>, RpcError> {
        self.client.workspaces().settings(id).await
    }
    /// Loaded workspace skill metadata.
    pub async fn workspace_skills(&self, id: &str) -> Result<Vec<g::LoadedSkill>, RpcError> {
        self.client.skills().list(id, None).await
    }
    /// Mark the selected conversation as actively viewed and read, once per open intent.
    pub async fn view_session(&self, id: &str, workspace: &str) -> Result<(), RpcError> {
        self.client
            .sessions()
            .command(
                id,
                &g::SessionCommand::SetActiveViewing {
                    workspace_id: workspace.into(),
                },
            )
            .await?;
        self.client
            .sessions()
            .command(id, &g::SessionCommand::MarkRead)
            .await?;
        Ok(())
    }
    /// Authoritative unread count at bootstrap.
    pub async fn unread_snapshot(&self) -> Result<g::UnreadSummary, RpcError> {
        self.client.sessions().unread_summary().await
    }
    /// Invalidation events for the safe connection catalog.
    pub fn connection_events(&self) -> async_channel::Receiver<Result<(), RpcError>> {
        self.client.events().connections_changed()
    }
    /// Workspace-local skill changes.
    pub fn skill_events(
        &self,
    ) -> async_channel::Receiver<Result<(String, Vec<g::LoadedSkill>), RpcError>> {
        self.client.events().skills_changed()
    }
    /// Global unread summary changes.
    pub fn unread_events(&self) -> async_channel::Receiver<Result<g::UnreadSummary, RpcError>> {
        self.client.events().unread_summary()
    }
    /// Generated summaries for the formal store.
    pub async fn sessions_generated(&self) -> Result<Vec<g::Session>, RpcError> {
        self.client.sessions().list().await
    }
    /// Create directly into the generated store contract.
    pub async fn create_session_generated(
        &self,
        workspace: &str,
        working_directory: Option<&str>,
    ) -> Result<g::Session, RpcError> {
        self.client
            .sessions()
            .create(
                workspace,
                Some(&g::CreateSessionOptions {
                    permission_mode: Some(g::PermissionMode::Safe),
                    working_directory: working_directory.map(str::to_owned),
                    ..Default::default()
                }),
            )
            .await
    }
    /// Send attachment-capable optimistic work once; uploads/picker remain controller concerns.
    pub async fn send_with_attachments(
        &self,
        session: &str,
        text: &str,
        optimistic_id: &str,
        attachments: &[g::FileAttachment],
        stored: &[g::StoredAttachment],
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
                    attachments: Some(attachments),
                    stored_attachments: Some(stored),
                    options: Some(&options),
                },
            )
            .await?;
        Ok(SendAck {
            accepted: ack.accepted,
            message_id: ack.message_id,
        })
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
    /// Cancel quietly before orderly shutdown, without retrying the mutation.
    pub async fn cancel_for_shutdown(&self, session: &str) -> Result<(), RpcError> {
        self.client.sessions().cancel(session, Some(true)).await
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
    /// Raw arguments retain future tags; window ingress performs explicit formal decoding.
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
