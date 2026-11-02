//! Explicit secret exposure happens only during requested wire serialization.
use agentkit_protocol::generated as g;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, Serializer};
fn secret<S: Serializer>(value: &SecretString, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(value.expose_secret())
}
fn optional_secret<S: Serializer>(value: &Option<SecretString>, s: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => s.serialize_some(value.expose_secret()),
        None => s.serialize_none(),
    }
}
macro_rules! redacted{($($ty:ty),*$(,)?)=>{$(impl std::fmt::Debug for $ty{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str(concat!(stringify!($ty)," { [REDACTED] }"))}})*}}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSetup {
    pub slug: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "optional_secret"
    )]
    pub credential: Option<SecretString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Option<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pi_auth_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_selection_mode: Option<g::ModelSelectionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_endpoint: Option<g::CustomEndpointConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_identity: Option<g::ClaudeOAuthIdentityDto>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionParams {
    provider: Pi,
    #[serde(serialize_with = "secret")]
    pub api_key: SecretString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pi_auth_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_endpoint: Option<g::CustomEndpointConfig>,
}
#[derive(Serialize)]
enum Pi {
    #[serde(rename = "pi")]
    Pi,
}
impl TestConnectionParams {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            provider: Pi::Pi,
            api_key,
            connection_slug: None,
            base_url: None,
            model: None,
            pi_auth_provider: None,
            custom_endpoint: None,
        }
    }
}
#[derive(Deserialize)]
pub struct ClaudeExchange {
    pub success: bool,
    #[serde(default)]
    pub token: Option<SecretString>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub identity: Option<g::ClaudeOAuthIdentityDto>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStart {
    pub success: bool,
    #[serde(default)]
    pub auth_url: Option<SecretString>,
    #[serde(default)]
    pub error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptFlow {
    pub auth_url: SecretString,
    pub state: SecretString,
    pub flow_id: SecretString,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptComplete {
    #[serde(serialize_with = "secret")]
    pub flow_id: SecretString,
    #[serde(serialize_with = "secret")]
    pub code: SecretString,
    #[serde(serialize_with = "secret")]
    pub state: SecretString,
}
#[derive(Default, Serialize)]
pub struct ChatGptCancel {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "optional_secret"
    )]
    pub state: Option<SecretString>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProxy {
    pub enabled: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "optional_secret"
    )]
    pub http_proxy: Option<SecretString>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "optional_secret"
    )]
    pub https_proxy: Option<SecretString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_proxy: Option<String>,
}
redacted!(
    ConnectionSetup,
    TestConnectionParams,
    ClaudeExchange,
    ClaudeStart,
    ChatGptFlow,
    ChatGptComplete,
    ChatGptCancel,
    NetworkProxy
);
