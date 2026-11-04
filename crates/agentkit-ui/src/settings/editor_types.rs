use secrecy::SecretString;
/// Credential transport selected by the user, independent of backend display names.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    /// Bundled API-key provider.
    Preset,
    /// Custom OpenAI protocol.
    OpenAi,
    /// Custom Anthropic protocol.
    Anthropic,
    /// Local keyless Ollama.
    Ollama,
}
/// Explicit form submission; secrets are memory-only and never implement Debug.
#[derive(Clone)]
pub struct ConnectionValues {
    /// Stable connection identity assigned by the controller.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Form kind.
    pub kind: ConnectionKind,
    /// Preset provider ID.
    pub provider: String,
    /// Endpoint URL, empty for presets.
    pub url: String,
    /// Requested model IDs.
    pub models: Vec<String>,
    /// Default model ID.
    pub model: String,
    /// Explicit replacement only; None keeps the backend credential reference.
    pub credential: Option<SecretString>,
    /// Update an existing record.
    pub existing: bool,
}
/// Connection editor actions with no RPC or persistence behavior.
#[derive(Clone)]
pub enum EditorIntent {
    /// Save reviewed values.
    Save(ConnectionValues),
    /// Test without persisting.
    Test(ConnectionValues),
    /// Fetch provider model metadata.
    Provider {
        /// Requested preset provider.
        provider: String,
        /// Editor configuration generation.
        generation: u64,
    },
}

impl ConnectionKind {
    /// Preserve the stored wire protocol when opening an existing connection.
    pub fn from_connection(s: &super::g::LlmConnectionWithStatus) -> Self {
        use super::g;
        if s.provider_type == g::LlmConnectionWithStatusProviderType::Pi {
            Self::Preset
        } else if s
            .custom_endpoint
            .as_ref()
            .is_some_and(|e| e.api == g::CustomEndpointConfigApi::AnthropicMessages)
        {
            Self::Anthropic
        } else if s.auth_type == g::LlmConnectionWithStatusAuthType::None {
            Self::Ollama
        } else {
            Self::OpenAi
        }
    }
}
