use super::super::{
    secrets::{ConnectionSetup, TestConnectionParams},
    types::{Provider, ProviderModels},
};
use super::*;
impl WorkbenchApi {
    /// Read the backend's built-in permissions as a typed, read-only projection.
    pub async fn builtin_permissions(
        &self,
    ) -> Result<super::super::types::DefaultPermissions, RpcError> {
        self.client.settings().default_permissions().await
    }
    /// Explicitly change defaults for future sessions in one workspace.
    pub async fn workspace_permission_mode(
        &self,
        workspace: &str,
        mode: g::PermissionMode,
    ) -> Result<(), RpcError> {
        self.client
            .workspaces()
            .update(
                workspace,
                &super::super::types::WorkspaceSetting::PermissionMode(mode),
            )
            .await
    }
    /// Persist a first-run deferral without saving credentials.
    pub async fn defer_setup(&self) -> Result<(), RpcError> {
        self.client.oauth().defer_setup().await.map(|_| ())
    }

    /// Safe provider presets; no credential retrieval is exposed.
    pub async fn connection_providers(&self) -> Result<Vec<Provider>, RpcError> {
        self.client.connections().api_key_providers().await
    }
    /// Models advertised by an API-key provider.
    pub async fn provider_models(&self, provider: &str) -> Result<ProviderModels, RpcError> {
        self.client.connections().provider_models(provider).await
    }
    /// Test an unsaved draft without persisting configuration or credentials.
    pub async fn test_connection_draft(
        &self,
        params: &TestConnectionParams,
    ) -> Result<(), RpcError> {
        self.client
            .connections()
            .test_setup(params)
            .await
            .map(|_| ())
    }
    /// Store credentials through the backend then preserve reviewed display metadata.
    pub async fn save_connection(
        &self,
        setup: &ConnectionSetup,
        metadata: &g::LlmConnection,
    ) -> Result<(), RpcError> {
        let previous = if setup.update_only == Some(true) {
            self.client
                .connections()
                .list_with_status()
                .await?
                .into_iter()
                .find(|c| c.slug == metadata.slug)
        } else {
            None
        };
        self.client.connections().setup(setup).await?;
        let canonical = self
            .client
            .connections()
            .list_with_status()
            .await?
            .into_iter()
            .find(|c| c.slug == metadata.slug)
            .ok_or_else(|| super::super::business_error("Connection disappeared".into()))?;
        let canonical: g::LlmConnection = serde_json::from_value(serde_json::to_value(canonical)?)?;
        let mut merged = metadata.clone();
        merged.model_selection_mode = merged
            .model_selection_mode
            .or(canonical.model_selection_mode);
        merged.mid_stream_behavior = merged.mid_stream_behavior.or(canonical.mid_stream_behavior);
        merged.provider_type = canonical.provider_type;
        merged.pi_auth_provider = canonical.pi_auth_provider;
        let same_endpoint = previous.as_ref().is_some_and(|c| {
            c.base_url == metadata.base_url
                && c.custom_endpoint.as_ref().map(|e| e.api)
                    == metadata.custom_endpoint.as_ref().map(|e| e.api)
        });
        if !same_endpoint || setup.credential.is_some() {
            merged.auth_type = canonical.auth_type;
        }
        self.client.connections().save(&merged).await.map(|_| ())
    }
    /// Update non-secret metadata of an already authenticated subscription.
    pub async fn save_connection_metadata(
        &self,
        metadata: &g::LlmConnection,
    ) -> Result<(), RpcError> {
        if !self
            .client
            .connections()
            .list_with_status()
            .await?
            .iter()
            .any(|c| c.slug == metadata.slug)
        {
            return Err(super::super::business_error(
                "Connection disappeared".into(),
            ));
        }
        self.client.connections().save(metadata).await.map(|_| ())
    }
    /// Test a persisted connection using its backend credential reference.
    pub async fn test_connection(&self, id: &str) -> Result<(), RpcError> {
        self.client.connections().test(id).await?;
        let connection = self
            .client
            .connections()
            .list_with_status()
            .await?
            .into_iter()
            .find(|c| c.slug == id)
            .ok_or_else(|| super::super::business_error("Connection disappeared".into()))?;
        // OAuth acquisition and live subscription probes follow the dedicated OAuth tasks.
        if connection.auth_type == g::LlmConnectionWithStatusAuthType::Oauth {
            return Ok(());
        }
        let mut params = TestConnectionParams::new("".into());
        params.connection_slug = Some(id.into());
        params.base_url = connection.base_url;
        params.model = connection.default_model;
        params.pi_auth_provider = connection.pi_auth_provider.or_else(|| {
            connection.custom_endpoint.as_ref().map(|e| {
                if e.api == g::CustomEndpointConfigApi::AnthropicMessages {
                    "anthropic".into()
                } else {
                    "openai".into()
                }
            })
        });
        params.custom_endpoint = connection.custom_endpoint;
        self.client
            .connections()
            .test_setup(&params)
            .await
            .map(|_| ())
    }
    /// Select the default connection for sessions without an override.
    pub async fn default_connection(&self, id: &str) -> Result<(), RpcError> {
        self.client.connections().set_default(id).await.map(|_| ())
    }
    /// Refresh a persisted model catalog.
    pub async fn refresh_connection_models(&self, id: &str) -> Result<(), RpcError> {
        self.client
            .connections()
            .refresh_models(id)
            .await
            .map(|_| ())
    }
    /// Remove a confirmed connection and let the backend manage shared credential references.
    pub async fn delete_connection(&self, id: &str) -> Result<(), RpcError> {
        self.client.connections().delete(id).await.map(|_| ())
    }
    /// Global defaults for future sessions.
    pub async fn model_defaults(&self) -> Result<(g::ThinkingLevel, Option<u32>), RpcError> {
        Ok((
            self.client.settings().default_thinking().await?,
            self.client.settings().auto_compact_window().await?,
        ))
    }
    /// Persist the default reasoning level.
    pub async fn default_thinking(&self, level: g::ThinkingLevel) -> Result<(), RpcError> {
        self.client
            .settings()
            .set_default_thinking(level)
            .await
            .map(|_| ())
    }
    /// Persist optional context compaction threshold in tokens.
    pub async fn auto_compact(&self, tokens: Option<u32>) -> Result<(), RpcError> {
        self.client
            .settings()
            .set_auto_compact_window(tokens)
            .await
            .map(|_| ())
    }
    /// Keep the backend's send-key preference in sync with the native client.
    pub async fn send_key(&self, key: super::super::SendMessageKey) -> Result<(), RpcError> {
        self.client.settings().set_send_message_key(key).await
    }
}

impl WorkbenchApi {
    /// Read saved custom proxy settings; URLs have redacted diagnostics.
    pub async fn network_proxy(&self) -> Result<super::super::secrets::NetworkProxy, RpcError> {
        self.client.settings().network_proxy().await
    }
    /// Persist custom routing and return the explicit backend restart acknowledgement.
    pub async fn save_network_proxy(
        &self,
        settings: &super::super::secrets::NetworkProxy,
    ) -> Result<super::super::types::NetworkProxyUpdate, RpcError> {
        self.client.settings().set_network_proxy(settings).await
    }
    /// Read the persisted keep-awake preference, without acquiring a power assertion.
    pub async fn keep_awake(&self) -> Result<bool, RpcError> {
        self.client.settings().keep_awake().await
    }
    /// Save the preference; the native client owns actual OS power management.
    pub async fn set_keep_awake(&self, enabled: bool) -> Result<(), RpcError> {
        self.client.settings().set_keep_awake(enabled).await
    }
}
