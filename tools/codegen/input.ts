// Explicit, reviewed wire surface. Never import upstream application code.
export type { Session, SessionEvent, PermissionRequest, CreateSessionOptions, SendMessageOptions, SessionCommand, FileAttachment } from '../../backend/mkagent/packages/shared/src/protocol/index.ts';
export type { Message, MessageRole, ToolStatus, Workspace, StoredAttachment } from '../../backend/mkagent/packages/core/src/types/index.ts';
export type { PermissionMode } from '../../backend/mkagent/packages/shared/src/agent/mode-types.ts';
export type { ThinkingLevel } from '../../backend/mkagent/packages/shared/src/agent/thinking-levels.ts';
