//! Typed settings RPC operations.
use super::*;
pub struct Settings<'a>(pub(super) &'a RpcClient);
use super::secrets::NetworkProxy;
impl Settings<'_> {
    pub async fn default_thinking(&self) -> Result<g::ThinkingLevel, RpcError> {
        execute(self.0, p::SETTINGS_DEFAULT_THINKING, vec![]).await
    }
    pub async fn set_default_thinking(
        &self,
        level: g::ThinkingLevel,
    ) -> Result<ActionResult, RpcError> {
        action(self.0, p::SETTINGS_SET_DEFAULT_THINKING, vec![json!(level)]).await
    }
    pub async fn auto_compact_window(&self) -> Result<Option<u32>, RpcError> {
        execute(self.0, p::SETTINGS_AUTO_COMPACT_WINDOW, vec![]).await
    }
    pub async fn set_auto_compact_window(
        &self,
        tokens: Option<u32>,
    ) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::SETTINGS_SET_AUTO_COMPACT_WINDOW,
            vec![json!(tokens)],
        )
        .await
    }
    pub async fn send_message_key(&self) -> Result<SendMessageKey, RpcError> {
        execute(self.0, p::SETTINGS_SEND_MESSAGE_KEY, vec![]).await
    }
    pub async fn set_send_message_key(&self, key: SendMessageKey) -> Result<(), RpcError> {
        execute(self.0, p::SETTINGS_SET_SEND_MESSAGE_KEY, vec![json!(key)]).await
    }
    pub async fn keep_awake(&self) -> Result<bool, RpcError> {
        execute(self.0, p::SETTINGS_KEEP_AWAKE, vec![]).await
    }
    pub async fn set_keep_awake(&self, value: bool) -> Result<(), RpcError> {
        execute(self.0, p::SETTINGS_SET_KEEP_AWAKE, vec![json!(value)]).await
    }
    pub async fn network_proxy(&self) -> Result<NetworkProxy, RpcError> {
        execute(self.0, p::SETTINGS_NETWORK_PROXY, vec![]).await
    }
    pub async fn set_network_proxy(
        &self,
        value: &NetworkProxy,
    ) -> Result<NetworkProxyUpdate, RpcError> {
        execute(
            self.0,
            p::SETTINGS_SET_NETWORK_PROXY,
            vec![serde_json::to_value(value)?],
        )
        .await
    }
    pub async fn disable_browser(&self) -> Result<(), RpcError> {
        execute(self.0, p::SETTINGS_DISABLE_BROWSER, vec![json!(false)]).await
    }
    pub async fn default_permissions(&self) -> Result<DefaultPermissions, RpcError> {
        execute(self.0, p::SETTINGS_DEFAULT_PERMISSIONS, vec![]).await
    }
}
