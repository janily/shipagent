// Reviewed workaround for extractor 2.9.0's inline ImportType parser.
// Field signatures are compared to protocol/dto.ts during every extraction.
import type { BoundPr, PrMonitorState } from '../../../backend/mkagent/packages/shared/src/protocol/pr-watch.ts';
import type { ChatStarter } from '../../../backend/mkagent/packages/shared/src/workspaces/types.ts';
import type { PermissionMode } from '../../../backend/mkagent/packages/shared/src/agent/mode-types.ts';
import type { ThinkingLevel } from '../../../backend/mkagent/packages/shared/src/agent/thinking-levels.ts';
export interface WorkspaceSettings {
  codingMode?: boolean;
  boundPrs?: BoundPr[];
  prMonitors?: Record<string, PrMonitorState>;
  chatStarters?: ChatStarter[];
  name?: string;
  model?: string;
  permissionMode?: PermissionMode;
  cyclablePermissionModes?: PermissionMode[];
  thinkingLevel?: ThinkingLevel;
  workingDirectory?: string;
  defaultLlmConnection?: string;
}
