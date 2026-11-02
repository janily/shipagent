//! Typed connections RPC operations.
use super::*;
pub struct Connections<'a>(pub(super) &'a RpcClient);
use super::secrets::*;
impl Connections<'_> {
    pub async fn list_with_status(&self) -> Result<Vec<g::LlmConnectionWithStatus>, RpcError> {
        execute(self.0, p::CONNECTIONS_LIST_WITH_STATUS, vec![]).await
    }
    pub async fn save(&self, connection: &g::LlmConnection) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::CONNECTIONS_SAVE,
            vec![serde_json::to_value(connection)?],
        )
        .await
    }
    pub async fn delete(&self, slug: &str) -> Result<ActionResult, RpcError> {
        action(self.0, p::CONNECTIONS_DELETE, vec![json!(slug)]).await
    }
    pub async fn test(&self, slug: &str) -> Result<ActionResult, RpcError> {
        action(self.0, p::CONNECTIONS_TEST, vec![json!(slug)]).await
    }
    pub async fn set_default(&self, slug: &str) -> Result<ActionResult, RpcError> {
        action(self.0, p::CONNECTIONS_SET_DEFAULT, vec![json!(slug)]).await
    }
    pub async fn set_workspace_default(
        &self,
        workspace: &str,
        slug: Option<&str>,
    ) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::CONNECTIONS_SET_WORKSPACE_DEFAULT,
            vec![json!(workspace), json!(slug)],
        )
        .await
    }
    pub async fn refresh_models(&self, slug: &str) -> Result<ActionResult, RpcError> {
        action(self.0, p::CONNECTIONS_REFRESH_MODELS, vec![json!(slug)]).await
    }
    pub async fn setup(&self, setup: &ConnectionSetup) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::CONNECTIONS_SETUP,
            vec![serde_json::to_value(setup)?],
        )
        .await
    }
    pub async fn test_setup(
        &self,
        params: &TestConnectionParams,
    ) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::CONNECTIONS_TEST_SETUP,
            vec![serde_json::to_value(params)?],
        )
        .await
    }
    pub async fn api_key_providers(&self) -> Result<Vec<Provider>, RpcError> {
        execute(self.0, p::CONNECTIONS_API_KEY_PROVIDERS, vec![]).await
    }
    pub async fn provider_base_url(&self, provider: &str) -> Result<Option<String>, RpcError> {
        execute(
            self.0,
            p::CONNECTIONS_PROVIDER_BASE_URL,
            vec![json!(provider)],
        )
        .await
    }
    pub async fn provider_models(&self, provider: &str) -> Result<ProviderModels, RpcError> {
        execute(
            self.0,
            p::CONNECTIONS_PROVIDER_MODELS,
            vec![json!(provider)],
        )
        .await
    }
}
