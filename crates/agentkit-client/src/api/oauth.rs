//! Typed oauth RPC operations.
use super::*;
pub struct Oauth<'a>(pub(super) &'a RpcClient);
use super::secrets::*;
use secrecy::{ExposeSecret, SecretString};
impl Oauth<'_> {
    pub async fn start_chatgpt(&self, slug: &str) -> Result<ChatGptFlow, RpcError> {
        execute(self.0, p::OAUTH_START_CHATGPT, vec![json!(slug)]).await
    }
    pub async fn complete_chatgpt(&self, args: &ChatGptComplete) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::OAUTH_COMPLETE_CHATGPT,
            vec![serde_json::to_value(args)?],
        )
        .await
    }
    pub async fn cancel_chatgpt(
        &self,
        args: Option<&ChatGptCancel>,
    ) -> Result<ActionResult, RpcError> {
        let mut params = vec![];
        optional(&mut params, args)?;
        action(self.0, p::OAUTH_CANCEL_CHATGPT, params).await
    }
    pub async fn chatgpt_status(&self, slug: &str) -> Result<AuthStatus, RpcError> {
        execute(self.0, p::OAUTH_CHATGPT_STATUS, vec![json!(slug)]).await
    }
    pub async fn logout_chatgpt(&self, slug: &str) -> Result<ActionResult, RpcError> {
        action(self.0, p::OAUTH_LOGOUT_CHATGPT, vec![json!(slug)]).await
    }
    pub async fn start_claude(&self) -> Result<ClaudeStart, RpcError> {
        let value: ClaudeStart = execute(self.0, p::OAUTH_START_CLAUDE, vec![]).await?;
        if !value.success {
            return Err(business_error(
                value.error.unwrap_or_else(|| "Claude OAuth failed".into()),
            ));
        }
        Ok(value)
    }
    pub async fn exchange_claude(
        &self,
        code: &SecretString,
        slug: &str,
    ) -> Result<ClaudeExchange, RpcError> {
        let value: ClaudeExchange = execute(
            self.0,
            p::OAUTH_EXCHANGE_CLAUDE,
            vec![json!(code.expose_secret()), json!(slug)],
        )
        .await?;
        if !value.success {
            return Err(business_error(
                value
                    .error
                    .unwrap_or_else(|| "Claude OAuth exchange failed".into()),
            ));
        }
        Ok(value)
    }
    pub async fn clear_claude_state(&self) -> Result<ActionResult, RpcError> {
        action(self.0, p::OAUTH_CLEAR_CLAUDE_STATE, vec![]).await
    }
    pub async fn auth_state(&self) -> Result<AuthState, RpcError> {
        execute(self.0, p::OAUTH_AUTH_STATE, vec![]).await
    }
    pub async fn defer_setup(&self) -> Result<ActionResult, RpcError> {
        action(self.0, p::OAUTH_DEFER_SETUP, vec![]).await
    }
}
