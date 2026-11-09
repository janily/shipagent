//! Reviewed v1 operation policies. Field privacy prevents ad-hoc timeout/retry construction outside this module.
use agentkit_protocol::constants::channels as c;
use std::time::Duration;
/// One immutable business RPC policy.
#[derive(Clone, Copy, Debug)]
pub struct Operation {
    channel: &'static str,
    seconds: u64,
    read_only: bool,
}
impl Operation {
    /// Exact upstream wire channel.
    pub const fn channel(self) -> &'static str {
        self.channel
    }
    /// Per-attempt deadline; a read can retry once on timeout.
    pub const fn timeout(self) -> Duration {
        Duration::from_secs(self.seconds)
    }
    /// Only idempotent reads opt into timeout retry.
    pub const fn is_read_only(self) -> bool {
        self.read_only
    }
}
pub(super) const SERVICE_HEALTH: Operation = Operation {
    channel: c::server::GET_HEALTH,
    seconds: 30,
    read_only: true,
};
pub(super) const SERVICE_VERSIONS: Operation = Operation {
    channel: c::system::VERSIONS,
    seconds: 30,
    read_only: true,
};
pub(super) const WORKSPACES_LIST: Operation = Operation {
    channel: c::workspaces::GET,
    seconds: 30,
    read_only: true,
};
pub(super) const WORKSPACES_CREATE: Operation = Operation {
    channel: c::workspaces::CREATE,
    seconds: 30,
    read_only: false,
};
pub(super) const WORKSPACES_CHECK_SLUG: Operation = Operation {
    channel: c::workspaces::CHECK_SLUG,
    seconds: 30,
    read_only: true,
};
pub(super) const WORKSPACES_REMOVE: Operation = Operation {
    channel: c::workspaces::REMOVE,
    seconds: 30,
    read_only: false,
};
pub(super) const WORKSPACES_SWITCH: Operation = Operation {
    channel: c::window::SWITCH_WORKSPACE,
    seconds: 30,
    read_only: false,
};
pub(super) const WORKSPACES_SETTINGS: Operation = Operation {
    channel: c::workspace::SETTINGS_GET,
    seconds: 30,
    read_only: true,
};
pub(super) const WORKSPACES_UPDATE: Operation = Operation {
    channel: c::workspace::SETTINGS_UPDATE,
    seconds: 30,
    read_only: false,
};
pub(super) const WORKSPACES_PERMISSIONS: Operation = Operation {
    channel: c::workspace::GET_PERMISSIONS,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_LIST: Operation = Operation {
    channel: c::sessions::GET,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_GET_MESSAGES: Operation = Operation {
    channel: c::sessions::GET_MESSAGES,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_CREATE: Operation = Operation {
    channel: c::sessions::CREATE,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_DELETE: Operation = Operation {
    channel: c::sessions::DELETE,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_SEND_MESSAGE: Operation = Operation {
    channel: c::sessions::SEND_MESSAGE,
    seconds: 300,
    read_only: false,
};
pub(super) const SESSIONS_CANCEL: Operation = Operation {
    channel: c::sessions::CANCEL,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_RESPOND_PERMISSION: Operation = Operation {
    channel: c::sessions::RESPOND_TO_PERMISSION,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_COMMAND: Operation = Operation {
    channel: c::sessions::COMMAND,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_PERMISSION_MODE: Operation = Operation {
    channel: c::sessions::GET_PERMISSION_MODE_STATE,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_PENDING_PLAN: Operation = Operation {
    channel: c::sessions::GET_PENDING_PLAN_EXECUTION,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_UNREAD_SUMMARY: Operation = Operation {
    channel: c::sessions::GET_UNREAD_SUMMARY,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_MARK_ALL_READ: Operation = Operation {
    channel: c::sessions::MARK_ALL_READ,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_SEARCH: Operation = Operation {
    channel: c::sessions::SEARCH_CONTENT,
    seconds: 15,
    read_only: true,
};
pub(super) const SESSIONS_BACKGROUND_OUTPUT: Operation = Operation {
    channel: c::sessions::GET_BACKGROUND_TASK_OUTPUT,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_STOP_BACKGROUND: Operation = Operation {
    channel: c::sessions::STOP_BACKGROUND_TASK,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_KILL_SHELL: Operation = Operation {
    channel: c::sessions::KILL_SHELL,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_FILES: Operation = Operation {
    channel: c::sessions::GET_FILES,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_WATCH_FILES: Operation = Operation {
    channel: c::sessions::WATCH_FILES,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_UNWATCH_FILES: Operation = Operation {
    channel: c::sessions::UNWATCH_FILES,
    seconds: 30,
    read_only: false,
};
pub(super) const SESSIONS_EXPORT: Operation = Operation {
    channel: c::sessions::EXPORT,
    seconds: 120,
    read_only: true,
};
pub(super) const SESSIONS_IMPORT: Operation = Operation {
    channel: c::sessions::IMPORT,
    seconds: 120,
    read_only: false,
};
pub(super) const SESSIONS_MODEL: Operation = Operation {
    channel: c::sessions::GET_MODEL,
    seconds: 30,
    read_only: true,
};
pub(super) const SESSIONS_SET_MODEL: Operation = Operation {
    channel: c::sessions::SET_MODEL,
    seconds: 30,
    read_only: false,
};
pub(super) const DRAFTS_GET: Operation = Operation {
    channel: c::drafts::GET,
    seconds: 30,
    read_only: true,
};
pub(super) const DRAFTS_SET: Operation = Operation {
    channel: c::drafts::SET,
    seconds: 30,
    read_only: false,
};
pub(super) const DRAFTS_DELETE: Operation = Operation {
    channel: c::drafts::DELETE,
    seconds: 30,
    read_only: false,
};
pub(super) const FILES_READ_TEXT: Operation = Operation {
    channel: c::file::READ,
    seconds: 30,
    read_only: true,
};
pub(super) const FILES_STORE_ATTACHMENT: Operation = Operation {
    channel: c::file::STORE_ATTACHMENT,
    seconds: 120,
    read_only: false,
};
pub(super) const FILES_READ_ATTACHMENT: Operation = Operation {
    channel: c::file::READ_ATTACHMENT,
    seconds: 30,
    read_only: true,
};
pub(super) const FILES_READ_DATA_URL: Operation = Operation {
    channel: c::file::READ_DATA_URL,
    seconds: 30,
    read_only: true,
};
pub(super) const FILES_READ_PREVIEW_DATA_URL: Operation = Operation {
    channel: c::file::READ_PREVIEW_DATA_URL,
    seconds: 30,
    read_only: true,
};
pub(super) const FILES_SEARCH: Operation = Operation {
    channel: c::fs::SEARCH,
    seconds: 30,
    read_only: true,
};
pub(super) const FILES_LIST_DIRECTORY: Operation = Operation {
    channel: c::fs::LIST_DIRECTORY,
    seconds: 30,
    read_only: true,
};
pub(super) const SKILLS_LIST: Operation = Operation {
    channel: c::skills::GET,
    seconds: 30,
    read_only: true,
};
pub(super) const SKILLS_FILES: Operation = Operation {
    channel: c::skills::GET_FILES,
    seconds: 30,
    read_only: true,
};
pub(super) const SKILLS_DELETE: Operation = Operation {
    channel: c::skills::DELETE,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_LIST_WITH_STATUS: Operation = Operation {
    channel: c::llm_connections::LIST_WITH_STATUS,
    seconds: 30,
    read_only: true,
};
pub(super) const CONNECTIONS_SAVE: Operation = Operation {
    channel: c::llm_connections::SAVE,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_DELETE: Operation = Operation {
    channel: c::llm_connections::DELETE,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_TEST: Operation = Operation {
    channel: c::llm_connections::TEST,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_SET_DEFAULT: Operation = Operation {
    channel: c::llm_connections::SET_DEFAULT,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_SET_WORKSPACE_DEFAULT: Operation = Operation {
    channel: c::llm_connections::SET_WORKSPACE_DEFAULT,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_REFRESH_MODELS: Operation = Operation {
    channel: c::llm_connections::REFRESH_MODELS,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_SETUP: Operation = Operation {
    channel: c::settings::SETUP_LLM_CONNECTION,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_TEST_SETUP: Operation = Operation {
    channel: c::settings::TEST_LLM_CONNECTION_SETUP,
    seconds: 30,
    read_only: false,
};
pub(super) const CONNECTIONS_API_KEY_PROVIDERS: Operation = Operation {
    channel: c::pi::GET_API_KEY_PROVIDERS,
    seconds: 30,
    read_only: true,
};
pub(super) const CONNECTIONS_PROVIDER_BASE_URL: Operation = Operation {
    channel: c::pi::GET_PROVIDER_BASE_URL,
    seconds: 30,
    read_only: true,
};
pub(super) const CONNECTIONS_PROVIDER_MODELS: Operation = Operation {
    channel: c::pi::GET_PROVIDER_MODELS,
    seconds: 30,
    read_only: true,
};
pub(super) const OAUTH_START_CHATGPT: Operation = Operation {
    channel: c::chatgpt::START_OAUTH,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_COMPLETE_CHATGPT: Operation = Operation {
    channel: c::chatgpt::COMPLETE_OAUTH,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_CANCEL_CHATGPT: Operation = Operation {
    channel: c::chatgpt::CANCEL_OAUTH,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_CHATGPT_STATUS: Operation = Operation {
    channel: c::chatgpt::GET_AUTH_STATUS,
    seconds: 30,
    read_only: true,
};
pub(super) const OAUTH_LOGOUT_CHATGPT: Operation = Operation {
    channel: c::chatgpt::LOGOUT,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_START_CLAUDE: Operation = Operation {
    channel: c::onboarding::START_CLAUDE_OAUTH,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_EXCHANGE_CLAUDE: Operation = Operation {
    channel: c::onboarding::EXCHANGE_CLAUDE_CODE,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_CLEAR_CLAUDE_STATE: Operation = Operation {
    channel: c::onboarding::CLEAR_CLAUDE_OAUTH_STATE,
    seconds: 30,
    read_only: false,
};
pub(super) const OAUTH_AUTH_STATE: Operation = Operation {
    channel: c::onboarding::GET_AUTH_STATE,
    seconds: 30,
    read_only: true,
};
pub(super) const OAUTH_DEFER_SETUP: Operation = Operation {
    channel: c::onboarding::DEFER_SETUP,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_DEFAULT_THINKING: Operation = Operation {
    channel: c::settings::GET_DEFAULT_THINKING_LEVEL,
    seconds: 30,
    read_only: true,
};
pub(super) const SETTINGS_SET_DEFAULT_THINKING: Operation = Operation {
    channel: c::settings::SET_DEFAULT_THINKING_LEVEL,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_AUTO_COMPACT_WINDOW: Operation = Operation {
    channel: c::settings::GET_AUTO_COMPACT_WINDOW,
    seconds: 30,
    read_only: true,
};
pub(super) const SETTINGS_SET_AUTO_COMPACT_WINDOW: Operation = Operation {
    channel: c::settings::SET_AUTO_COMPACT_WINDOW,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_SEND_MESSAGE_KEY: Operation = Operation {
    channel: c::input::GET_SEND_MESSAGE_KEY,
    seconds: 30,
    read_only: true,
};
pub(super) const SETTINGS_SET_SEND_MESSAGE_KEY: Operation = Operation {
    channel: c::input::SET_SEND_MESSAGE_KEY,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_KEEP_AWAKE: Operation = Operation {
    channel: c::power::GET_KEEP_AWAKE,
    seconds: 30,
    read_only: true,
};
pub(super) const SETTINGS_SET_KEEP_AWAKE: Operation = Operation {
    channel: c::power::SET_KEEP_AWAKE,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_NETWORK_PROXY: Operation = Operation {
    channel: c::settings::GET_NETWORK_PROXY,
    seconds: 30,
    read_only: true,
};
pub(super) const SETTINGS_SET_NETWORK_PROXY: Operation = Operation {
    channel: c::settings::SET_NETWORK_PROXY,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_DISABLE_BROWSER: Operation = Operation {
    channel: c::tools::SET_BROWSER_TOOL_ENABLED,
    seconds: 30,
    read_only: false,
};
pub(super) const SETTINGS_DEFAULT_PERMISSIONS: Operation = Operation {
    channel: c::permissions::GET_DEFAULTS,
    seconds: 30,
    read_only: true,
};
/// All 79 v1 request channels, including unavailable-until-patched operations.
pub const V1_OPERATIONS: &[Operation] = &[
    SERVICE_HEALTH,
    SERVICE_VERSIONS,
    WORKSPACES_LIST,
    WORKSPACES_CREATE,
    WORKSPACES_CHECK_SLUG,
    WORKSPACES_REMOVE,
    WORKSPACES_SWITCH,
    WORKSPACES_SETTINGS,
    WORKSPACES_UPDATE,
    WORKSPACES_PERMISSIONS,
    SESSIONS_LIST,
    SESSIONS_GET_MESSAGES,
    SESSIONS_CREATE,
    SESSIONS_DELETE,
    SESSIONS_SEND_MESSAGE,
    SESSIONS_CANCEL,
    SESSIONS_RESPOND_PERMISSION,
    SESSIONS_COMMAND,
    SESSIONS_PERMISSION_MODE,
    SESSIONS_PENDING_PLAN,
    SESSIONS_UNREAD_SUMMARY,
    SESSIONS_MARK_ALL_READ,
    SESSIONS_SEARCH,
    SESSIONS_BACKGROUND_OUTPUT,
    SESSIONS_STOP_BACKGROUND,
    SESSIONS_KILL_SHELL,
    SESSIONS_FILES,
    SESSIONS_WATCH_FILES,
    SESSIONS_UNWATCH_FILES,
    SESSIONS_EXPORT,
    SESSIONS_IMPORT,
    SESSIONS_MODEL,
    SESSIONS_SET_MODEL,
    DRAFTS_GET,
    DRAFTS_SET,
    DRAFTS_DELETE,
    FILES_READ_TEXT,
    FILES_STORE_ATTACHMENT,
    FILES_READ_ATTACHMENT,
    FILES_READ_DATA_URL,
    FILES_READ_PREVIEW_DATA_URL,
    FILES_SEARCH,
    FILES_LIST_DIRECTORY,
    SKILLS_LIST,
    SKILLS_FILES,
    SKILLS_DELETE,
    CONNECTIONS_LIST_WITH_STATUS,
    CONNECTIONS_SAVE,
    CONNECTIONS_DELETE,
    CONNECTIONS_TEST,
    CONNECTIONS_SET_DEFAULT,
    CONNECTIONS_SET_WORKSPACE_DEFAULT,
    CONNECTIONS_REFRESH_MODELS,
    CONNECTIONS_SETUP,
    CONNECTIONS_TEST_SETUP,
    CONNECTIONS_API_KEY_PROVIDERS,
    CONNECTIONS_PROVIDER_BASE_URL,
    CONNECTIONS_PROVIDER_MODELS,
    OAUTH_START_CHATGPT,
    OAUTH_COMPLETE_CHATGPT,
    OAUTH_CANCEL_CHATGPT,
    OAUTH_CHATGPT_STATUS,
    OAUTH_LOGOUT_CHATGPT,
    OAUTH_START_CLAUDE,
    OAUTH_EXCHANGE_CLAUDE,
    OAUTH_CLEAR_CLAUDE_STATE,
    OAUTH_AUTH_STATE,
    OAUTH_DEFER_SETUP,
    SETTINGS_DEFAULT_THINKING,
    SETTINGS_SET_DEFAULT_THINKING,
    SETTINGS_AUTO_COMPACT_WINDOW,
    SETTINGS_SET_AUTO_COMPACT_WINDOW,
    SETTINGS_SEND_MESSAGE_KEY,
    SETTINGS_SET_SEND_MESSAGE_KEY,
    SETTINGS_KEEP_AWAKE,
    SETTINGS_SET_KEEP_AWAKE,
    SETTINGS_NETWORK_PROXY,
    SETTINGS_SET_NETWORK_PROXY,
    SETTINGS_DISABLE_BROWSER,
    SETTINGS_DEFAULT_PERMISSIONS,
];
