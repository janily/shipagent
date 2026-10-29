//! Typed phase-zero API. This facade owns positional wire arguments and timeouts.
use crate::{ClientOptions, ConnectionState, Endpoint, RpcClient, RpcError};
use agentkit_protocol::prototype::{Connection, SendAck, Session, Workspace};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::time::Duration;
/// Application-facing phase-zero RPC operations.
#[derive(Clone)]
pub struct WorkbenchApi {
    client: RpcClient,
}
impl WorkbenchApi {
    /// Bind an independent local connection to a workspace.
    pub fn connect(endpoint: Endpoint, workspace_id: String) -> Self {
        Self {
            client: RpcClient::connect(ClientOptions::local(
                endpoint.url,
                endpoint.token,
                workspace_id,
            )),
        }
    }
    /// Fetch configured workspaces before selecting a binding.
    pub async fn workspaces(&self) -> Result<Vec<Workspace>, RpcError> {
        self.client.invoke("workspaces:get", vec![]).await
    }
    /// Fetch summaries for this connection's workspace.
    pub async fn sessions(&self) -> Result<Vec<Session>, RpcError> {
        self.client.invoke("sessions:get", vec![]).await
    }
    /// Load persisted messages for a session.
    pub async fn session(&self, id: &str) -> Result<Session, RpcError> {
        self.client
            .invoke("sessions:getMessages", vec![json!(id)])
            .await
    }
    /// Decode an actual backend snapshot with the generated T-0-07 DTO surface.
    pub async fn session_generated(
        &self,
        id: &str,
    ) -> Result<agentkit_protocol::generated::Session, RpcError> {
        self.client
            .invoke("sessions:getMessages", vec![json!(id)])
            .await
    }
    /// Create a safe session; no permission escalation is performed.
    pub async fn create_session(
        &self,
        workspace: &str,
        working_directory: Option<&str>,
    ) -> Result<Session, RpcError> {
        let mut options = json!({"permissionMode":"safe"});
        if let Some(directory) = working_directory {
            options["workingDirectory"] = json!(directory);
        }
        self.client
            .invoke("sessions:create", vec![json!(workspace), options])
            .await
    }
    /// Submit a message with a stable optimistic identity and the documented long timeout.
    pub async fn send(
        &self,
        session: &str,
        text: &str,
        optimistic_id: &str,
    ) -> Result<SendAck, RpcError> {
        // Slots 2/3 are supplied as empty attachment arrays so options is positional argument 4.
        self.client
            .invoke_timeout(
                "sessions:sendMessage",
                vec![
                    json!(session),
                    json!(text),
                    json!([]),
                    json!([]),
                    json!({"optimisticMessageId":optimistic_id}),
                ],
                Duration::from_secs(300),
            )
            .await
    }
    /// Cancel the current backend round.
    pub async fn cancel(&self, session: &str) -> Result<(), RpcError> {
        self.client
            .invoke::<Value>("sessions:cancel", vec![json!(session)])
            .await
            .map(|_| ())
    }
    /// Send the user's explicit tool-permission decision.
    pub async fn respond_permission(
        &self,
        session: &str,
        request: &str,
        allowed: bool,
        always: bool,
    ) -> Result<(), RpcError> {
        self.client
            .invoke::<Value>(
                "sessions:respondToPermission",
                vec![
                    json!(session),
                    json!(request),
                    json!(allowed),
                    json!(always),
                ],
            )
            .await
            .map(|_| ())
    }
    /// Query connection authentication state, without retrieving credentials.
    pub async fn connections(&self) -> Result<Vec<Connection>, RpcError> {
        self.client
            .invoke("LLM_Connection:listWithStatus", vec![])
            .await
    }
    /// Configure a user-selected compatible endpoint. Used by the prototype connection form and tests.
    pub async fn setup_endpoint(
        &self,
        base_url: &str,
        model: &str,
        credential: Option<&SecretString>,
    ) -> Result<(), RpcError> {
        let mut setup = json!({"slug":"workbench-endpoint","baseUrl":base_url,"defaultModel":model,"models":[model],"customEndpoint":{"api":"openai-completions","supportsImages":false,"maxTokens":2048}});
        if let Some(key) = credential {
            setup["credential"] = json!(key.expose_secret());
        }
        let result: Value = self
            .client
            .invoke("settings:setupLlmConnection", vec![setup])
            .await?;
        if result["success"] != true {
            return Err(RpcError::Transport(
                result["error"]
                    .as_str()
                    .unwrap_or("Connection setup failed")
                    .into(),
            ));
        }
        self.client
            .invoke::<Value>(
                "LLM_Connection:setDefault",
                vec![json!("workbench-endpoint")],
            )
            .await?;
        Ok(())
    }
    /// Disable unsupported browser tooling on this backend.
    pub async fn disable_browser(&self) -> Result<(), RpcError> {
        self.client
            .invoke::<Value>("tools:setBrowserToolEnabled", vec![json!(false)])
            .await
            .map(|_| ())
    }
    /// Push-event stream. Each element is the upstream positional argument array.
    pub fn session_events(&self) -> async_channel::Receiver<Vec<Value>> {
        self.client.subscribe("session:event")
    }
    /// Current transport state.
    pub fn state(&self) -> ConnectionState {
        self.client.state()
    }
    /// Last terminal connection error.
    pub fn last_error(&self) -> Option<RpcError> {
        self.client.last_error()
    }
    /// Close the connection without retrying writes.
    pub fn close(&self) {
        self.client.close();
    }
}
