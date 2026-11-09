use agentkit_client::api::WorkbenchApi;
use agentkit_protocol::generated as g;
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;
/// Owned result: until the foreground accepts it, cancellation/window destruction
/// drops this guard and removes only the newly-created connection, never PKCE state.
pub(super) struct PendingConnection {
    api: WorkbenchApi,
    slug: String,
    catalog: Option<Vec<g::LlmConnectionWithStatus>>,
}
impl PendingConnection {
    pub(super) fn accept(mut self) -> Vec<g::LlmConnectionWithStatus> {
        self.catalog.take().unwrap_or_default()
    }
}
impl Drop for PendingConnection {
    fn drop(&mut self) {
        if self.catalog.is_some() {
            let api = self.api.clone();
            let slug = self.slug.clone();
            smol::spawn(async move {
                api.discard_claude(&slug).await;
            })
            .detach();
        }
    }
}
pub(super) struct Request {
    pub api: WorkbenchApi,
    pub slug: String,
    pub onboarding: bool,
    pub cancel: async_channel::Receiver<()>,
    pub code: async_channel::Receiver<SecretString>,
    pub ready: async_channel::Sender<()>,
    pub lease: std::sync::Arc<smol::lock::Mutex<()>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoginError {
    Busy,
    Browser,
    Cancelled,
    Timeout,
    Code,
    Backend,
}
pub(super) async fn login(
    request: Request,
    open: impl FnOnce(&str) -> Result<(), ()>,
) -> Result<PendingConnection, LoginError> {
    let Request {
        api,
        slug,
        onboarding,
        cancel,
        code,
        ready,
        lease,
    } = request;
    let _lease = lease.try_lock().ok_or(LoginError::Busy)?;
    let slug = slug.as_str();
    let result = async {
        let started = api.start_claude().await.map_err(|_| LoginError::Backend)?;
        if stopped(&cancel) {
            return Err(LoginError::Cancelled);
        }
        let url = started.auth_url.ok_or(LoginError::Browser)?;
        if !valid_url(url.expose_secret()) {
            return Err(LoginError::Browser);
        }
        open(url.expose_secret()).map_err(|_| LoginError::Browser)?;
        let _ = ready.send(()).await;
        let code = wait_code(code, cancel.clone(), Duration::from_secs(600)).await?;
        if stopped(&cancel) {
            return Err(LoginError::Cancelled);
        }
        if code.expose_secret().trim().is_empty() {
            return Err(LoginError::Code);
        }
        api.finish_claude(slug, &code)
            .await
            .map_err(|_| LoginError::Code)?;
        drop(code);
        if stopped(&cancel) {
            return Err(LoginError::Cancelled);
        }
        if onboarding {
            api.default_connection(slug)
                .await
                .map_err(|_| LoginError::Backend)?;
        }
        let catalog = api
            .connections_generated()
            .await
            .map_err(|_| LoginError::Backend)?;
        if !catalog.iter().any(|c| {
            c.slug == slug && c.is_authenticated && (!onboarding || c.is_default == Some(true))
        }) {
            return Err(LoginError::Backend);
        }
        Ok(catalog)
    }
    .await;
    let result = if stopped(&cancel) {
        Err(LoginError::Cancelled)
    } else {
        result
    };
    if result.is_err() {
        api.discard_claude(slug).await;
    }
    let cleared = api.clear_claude().await;
    // Cancellation can arrive while the final clear RPC is pending.
    if stopped(&cancel) && result.is_ok() {
        api.discard_claude(slug).await;
        return Err(LoginError::Cancelled);
    }
    if cleared.is_err() && result.is_ok() {
        api.discard_claude(slug).await;
        return Err(LoginError::Backend);
    }
    result.map(|catalog| PendingConnection {
        api,
        slug: slug.into(),
        catalog: Some(catalog),
    })
}
async fn wait_code(
    code: async_channel::Receiver<SecretString>,
    cancel: async_channel::Receiver<()>,
    timeout: Duration,
) -> Result<SecretString, LoginError> {
    smol::future::race(
        async { code.recv().await.map_err(|_| LoginError::Cancelled) },
        smol::future::race(
            async {
                let _ = cancel.recv().await;
                Err(LoginError::Cancelled)
            },
            async {
                smol::Timer::after(timeout).await;
                Err(LoginError::Timeout)
            },
        ),
    )
    .await
}
fn stopped(cancel: &async_channel::Receiver<()>) -> bool {
    cancel.is_closed() || cancel.try_recv().is_ok()
}
fn valid_url(raw: &str) -> bool {
    let Ok(u) = url::Url::parse(raw) else {
        return false;
    };
    let pairs: Vec<_> = u.query_pairs().collect();
    u.scheme() == "https"
        && u.host_str() == Some("claude.ai")
        && u.path() == "/oauth/authorize"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.fragment().is_none()
        && [
            "state",
            "redirect_uri",
            "code_challenge",
            "code_challenge_method",
        ]
        .iter()
        .all(|key| pairs.iter().filter(|(k, _)| k == key).count() == 1)
        && pairs.iter().any(|(k, v)| k == "state" && !v.is_empty())
        && pairs
            .iter()
            .any(|(k, v)| k == "code_challenge" && !v.is_empty())
        && pairs
            .iter()
            .any(|(k, v)| k == "code_challenge_method" && v == "S256")
        && pairs.iter().any(|(k, v)| {
            k == "redirect_uri" && v == "https://console.anthropic.com/oauth/code/callback"
        })
}
#[cfg(test)]
#[path = "claude_login_tests.rs"]
mod tests;
