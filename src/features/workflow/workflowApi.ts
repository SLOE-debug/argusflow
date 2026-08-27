import { invoke } from '@tauri-apps/api/core';

import type {
  AqlInspection,
  AutomationTarget,
  BackendCommandErrorCode,
  CommandError,
  FlowComponentDefinition,
  RunInputs,
  RunStarted,
  ValidationReport,
  WorkflowDefinition,
} from './contracts';
import { COMMAND_ERROR_CODES } from './contracts';

/** 后端当前允许返回的稳定命令错误码集合。 */
const commandErrorCodes = new Set<string>(COMMAND_ERROR_CODES);

/** 判断当前页面是否运行在拥有 Tauri IPC 的桌面 WebView 中。 */
export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined'
    && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

/** 请求 Runtime Planner 基于当前执行上下文生成只读 AQL Explain。 */
export function inspectAql(
  target: AutomationTarget,
): Promise<AqlInspection> {
  return invoke<AqlInspection>('inspect_aql', { target });
}

/** 请求后端校验工作流结构，并返回可定位到节点或边的问题。 */
export function validateWorkflow(
  workflow: WorkflowDefinition,
  components: ReadonlyArray<FlowComponentDefinition>,
): Promise<ValidationReport> {
  return invoke<ValidationReport>('validate_workflow', { workflow, components });
}

/** 请求后端启动工作流，成功时返回本次运行 ID；命令失败时 Promise 会拒绝。 */
export function runWorkflow(
  workflow: WorkflowDefinition,
  components: ReadonlyArray<FlowComponentDefinition>,
  inputs: RunInputs,
): Promise<RunStarted> {
  return invoke<RunStarted>('run_workflow', { workflow, components, inputs });
}

/** 将 Tauri 抛出的未知值归一化为界面可安全展示的命令错误。 */
export function normalizeCommandError(error: unknown): CommandError {
  if (isCommandError(error)) {
    return error;
  }

  return {
    code: 'unknown_error',
    message: error instanceof Error ? error.message : String(error),
    issues: [],
  };
}

function isCommandError(value: unknown): value is CommandError {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<CommandError>;
  return (
    isBackendCommandErrorCode(candidate.code) &&
    typeof candidate.message === 'string' &&
    Array.isArray(candidate.issues)
  );
}

/** 检查未知字符串是否属于 Rust `CommandErrorCode` 契约。 */
function isBackendCommandErrorCode(value: unknown): value is BackendCommandErrorCode {
  return typeof value === 'string' && commandErrorCodes.has(value);
}
