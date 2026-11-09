use agentkit_capabilities::oauth_chatgpt::{CallbackServer, LoginError};
use agentkit_client::api::{
    WorkbenchApi,
    secrets::{ChatGptCancel, ChatGptComplete},
};
use secrecy::{ExposeSecret, SecretString};
struct Cleanup {
    api: WorkbenchApi,
    state: SecretString,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let api = self.api.clone();
        let args = ChatGptCancel {
            state: Some(self.state.clone()),
        };
        smol::spawn(async move {
            let _ = api.cancel_chatgpt(Some(&args)).await;
        })
        .detach();
    }
}
/// Run one newly-created connection flow on a worker. The receiver closing also cancels.
/// The callback listener is acquired before creating server-owned PKCE state.
pub async fn login(
    api: WorkbenchApi,
    slug: &str,
    cancel: async_channel::Receiver<()>,
    page: String,
    open: impl FnOnce(&str) -> Result<(), ()>,
) -> Result<(), LoginError> {
    let listener = CallbackServer::bind().await?;
    let flow = api
        .start_chatgpt(slug)
        .await
        .map_err(|_| LoginError::Backend)?;
    let _cleanup = Cleanup {
        api: api.clone(),
        state: flow.state.clone(),
    };
    if cancelled(&cancel) {
        return Err(LoginError::Cancelled);
    }
    let url = url::Url::parse(flow.auth_url.expose_secret()).map_err(|_| LoginError::Browser)?;
    let pairs: Vec<_> = url.query_pairs().collect();
    if url.scheme() != "https"
        || url.host_str() != Some("auth.openai.com")
        || url.path() != "/oauth/authorize"
        || url.fragment().is_some()
        || pairs.iter().filter(|(k, _)| k == "redirect_uri").count() != 1
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || !pairs
            .iter()
            .any(|(k, v)| k == "state" && v == flow.state.expose_secret())
        || !pairs
            .iter()
            .any(|(k, v)| k == "redirect_uri" && v == "http://localhost:1455/auth/callback")
    {
        return Err(LoginError::Browser);
    }
    open(url.as_str()).map_err(|_| LoginError::Browser)?;
    let code = listener
        .receive(flow.state.expose_secret(), cancel.clone(), page)
        .await?;
    if cancelled(&cancel) {
        return Err(LoginError::Cancelled);
    }
    let result = api
        .complete_chatgpt(&ChatGptComplete {
            flow_id: flow.flow_id,
            code,
            state: flow.state,
        })
        .await;
    if cancelled(&cancel) {
        let _ = api.logout_chatgpt(slug).await;
        return Err(LoginError::Cancelled);
    }
    result.map_err(|_| LoginError::Backend)?;
    let finished = api.finish_chatgpt(slug).await;
    if cancelled(&cancel) {
        let _ = api.delete_connection(slug).await;
        let _ = api.logout_chatgpt(slug).await;
        return Err(LoginError::Cancelled);
    }
    finished.map_err(|_| LoginError::Backend)?;
    Ok(())
}
fn cancelled(cancel: &async_channel::Receiver<()>) -> bool {
    cancel.is_closed() || cancel.try_recv().is_ok()
}
#[cfg(test)]
#[path = "chatgpt_login_tests.rs"]
mod tests;
