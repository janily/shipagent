// Explicit, reviewed wire surface. Never import upstream application code.
export type { Session, SessionEvent, PermissionRequest, CreateSessionOptions, SendMessageOptions, SessionCommand, FileAttachment } from '../../backend/mkagent/packages/shared/src/protocol/index.ts';
export type { Message, MessageRole, ToolStatus, Workspace, StoredAttachment } from '../../backend/mkagent/packages/core/src/types/index.ts';
export type { PermissionMode } from '../../backend/mkagent/packages/shared/src/agent/mode-types.ts';
export type { ThinkingLevel } from '../../backend/mkagent/packages/shared/src/agent/thinking-levels.ts';
export type { PermissionModeState, DirectoryListingResult, SessionFile, FileSearchResult, SkillFile, SessionSearchResult, UnreadSummary, BackgroundTaskOutput, StopBackgroundTaskResult, RefreshTitleResult, LlmConnectionSetup, TestLlmConnectionParams, ClaudeOAuthResult, ClaudeOAuthIdentityDto } from '../../backend/mkagent/packages/shared/src/protocol/index.ts';
export type { PendingPlanExecution } from '../../backend/mkagent/packages/shared/src/sessions/types.ts';
export type { SessionBundle } from '../../backend/mkagent/packages/shared/src/sessions/bundle.ts';
export type { SessionDraft } from '../../backend/mkagent/packages/shared/src/config/storage.ts';
export type { LlmConnection, LlmConnectionWithStatus, CustomEndpointConfig, ModelSelectionMode } from '../../backend/mkagent/packages/shared/src/config/llm-connections.ts';
export type { NetworkProxySettings } from '../../backend/mkagent/packages/shared/src/config/types.ts';
export type { LoadedSkill } from '../../backend/mkagent/packages/shared/src/skills/types.ts';
export type { ServerHealth } from '../../backend/mkagent/packages/core/src/types/index.ts';
export type { WorkspaceSettings } from './overrides/api-input.ts';
