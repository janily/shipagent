//! AgentKit capabilities layer. See docs/spec/03 模块详细设计.md.
#![deny(missing_docs)]

/// Private native system notification delivery.
pub mod notifications;

/// Native reverse-RPC dispatch and validated operating-system actions.
pub mod client_caps;

/// Open a client-owned local file with the operating system association; call on a worker.
pub fn open_local_path(path: &std::path::Path) -> Result<(), String> {
    opener::open(path).map_err(|e| e.to_string())
}

/// Native loopback callback and cancellable ChatGPT browser login.
pub mod oauth_chatgpt;

/// Launch a previously validated browser authorization URL from a worker.
pub fn open_browser_url(url: &str) -> Result<(), oauth_chatgpt::LoginError> {
    let parsed = url::Url::parse(url).map_err(|_| oauth_chatgpt::LoginError::Browser)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("auth.openai.com")
        || parsed.path() != "/oauth/authorize"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return Err(oauth_chatgpt::LoginError::Browser);
    }
    opener::open_browser(url).map_err(|_| oauth_chatgpt::LoginError::Browser)
}

/// Open only the fixed Claude HTTPS authorization origin on a worker.
pub fn open_claude_url(raw: &str) -> Result<(), oauth_chatgpt::LoginError> {
    let u = url::Url::parse(raw).map_err(|_| oauth_chatgpt::LoginError::Browser)?;
    if u.scheme() != "https"
        || u.host_str() != Some("claude.ai")
        || u.path() != "/oauth/authorize"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.fragment().is_some()
    {
        return Err(oauth_chatgpt::LoginError::Browser);
    }
    opener::open_browser(raw).map_err(|_| oauth_chatgpt::LoginError::Browser)
}
