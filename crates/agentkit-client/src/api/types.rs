//! Reviewed wire shapes for anonymous upstream handler results.
use crate::RpcError;
use agentkit_protocol::generated as g;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
impl std::fmt::Debug for ActionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionResult")
            .field("success", &self.success)
            .finish_non_exhaustive()
    }
}
impl ActionResult {
    pub fn check(self) -> Result<Self, RpcError> {
        if self.success {
            Ok(self)
        } else {
            Err(business_error(
                self.error
                    .unwrap_or_else(|| "Backend operation failed".into()),
            ))
        }
    }
}
pub(crate) fn business_error(message: String) -> RpcError {
    RpcError::Wire(agentkit_protocol::WireError {
        code: agentkit_protocol::ErrorCode::HandlerError,
        message,
        data: None,
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendAck {
    #[serde(deserialize_with = "accepted")]
    pub accepted: bool,
    pub message_id: String,
}
fn accepted<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    if bool::deserialize(d)? {
        Ok(true)
    } else {
        Err(serde::de::Error::custom("Expected accepted:true"))
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Versions {
    pub node: String,
    #[serde(default)]
    pub chrome: Option<String>,
    #[serde(default)]
    pub electron: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlugCheck {
    pub exists: bool,
    pub path: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReadCount {
    pub count: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub session_id: String,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CopyPathResult {
    pub success: bool,
    #[serde(default)]
    pub path: Option<String>,
}
#[derive(Clone, Debug)]
pub enum CommandResult {
    Empty,
    Title(g::RefreshTitleResult),
    Path(CopyPathResult),
    Continued(ImportResult),
    /// Whether this caller acquired the pending plan execution claim.
    PlanDispatched(bool),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    Fork,
    Move,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    Agent,
    Shell,
    Workflow,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SendMessageKey {
    #[serde(rename = "enter")]
    Enter,
    #[serde(rename = "cmd-enter")]
    CmdEnter,
}
// Upstream z.infer schema in agent/mode-types.ts; no Zod runtime is loaded.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PermissionRule {
    Pattern(String),
    Commented {
        pattern: String,
        #[serde(default)]
        comment: Option<String>,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedHint {
    pub command: String,
    pub reason: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub try_instead: Option<Vec<String>>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub when_not_matching: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsConfig {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub allowed_bash_patterns: Option<Vec<PermissionRule>>,
    #[serde(default)]
    pub allowed_write_paths: Option<Vec<PermissionRule>>,
    #[serde(default)]
    pub blocked_tools: Option<Vec<PermissionRule>>,
    #[serde(default)]
    pub blocked_command_hints: Option<Vec<BlockedHint>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DefaultPermissions {
    #[serde(deserialize_with = "required_option")]
    pub config: Option<PermissionsConfig>,
    pub path: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Provider {
    pub key: String,
    pub label: String,
    pub placeholder: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    pub cost_input: f64,
    pub cost_output: f64,
    pub context_window: f64,
    pub reasoning: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModels {
    pub models: Vec<ProviderModel>,
    pub total_count: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupNeeds {
    pub is_fully_configured: bool,
    pub needs_billing_config: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub setup_needs: SetupNeeds,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub authenticated: bool,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub has_refresh_token: Option<bool>,
}
/// The backend validates exactly these nine workspace keys; PR state is read-only.
#[derive(Clone, Debug)]
pub enum WorkspaceSetting {
    CodingMode(bool),
    ChatStarters(Vec<g::WorkspaceSettingsChatStartersItem>),
    Name(String),
    Model(Option<String>),
    PermissionMode(g::PermissionMode),
    CyclablePermissionModes(Vec<g::PermissionMode>),
    ThinkingLevel(g::ThinkingLevel),
    WorkingDirectory(Option<String>),
    DefaultConnection(Option<String>),
}
impl WorkspaceSetting {
    pub(crate) fn wire(&self) -> (&'static str, serde_json::Value) {
        use serde_json::json;
        match self {
            Self::CodingMode(v) => ("codingMode", json!(v)),
            Self::ChatStarters(v) => ("chatStarters", json!(v)),
            Self::Name(v) => ("name", json!(v)),
            Self::Model(v) => ("model", json!(v)),
            Self::PermissionMode(v) => ("permissionMode", json!(v)),
            Self::CyclablePermissionModes(v) => ("cyclablePermissionModes", json!(v)),
            Self::ThinkingLevel(v) => ("thinkingLevel", json!(v)),
            Self::WorkingDirectory(v) => ("workingDirectory", json!(v)),
            Self::DefaultConnection(v) => ("defaultLlmConnection", json!(v)),
        }
    }
}
#[derive(Default)]
pub struct SendParts<'a> {
    pub attachments: Option<&'a [g::FileAttachment]>,
    pub stored_attachments: Option<&'a [g::StoredAttachment]>,
    pub options: Option<&'a g::SendMessageOptions>,
}

fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
/// No payload contract is defined upstream for these two non-emitting lifecycle channels.
/// Preserve their positional values until the backend defines one; never depend on them.
#[derive(Clone, Debug)]
pub struct ServerNotice {
    pub arguments: Vec<agentkit_protocol::RpcValue>,
}

/// Persisted headless proxy configuration; the current process keeps its old routing.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProxyUpdate {
    /// Restart the owned backend before treating the new routing as active.
    pub restart_required: bool,
}
