import { invoke } from '@tauri-apps/api/core';

import type {
  AqlInspection,
  AutomationTarget,
  BackendCommandErrorCode,
  CommandError,
  FlowComponentDefinition,
  RunInputs,
  RunDetails,
  RunManifest,
  RunStarted,
  RunTraceEvent,
  ValidationReport,
  WorkflowDefinition,
} from '../model/contracts';
import { COMMAND_ERROR_CODES } from '../model/contracts';

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

/** 按开始时间倒序读取本地运行索引。 */
export function listRuns(): Promise<RunManifest[]> {
  return invoke<RunManifest[]>('list_runs');
}

/** 读取一次运行及其执行时工作流快照。 */
export function getRun(runId: string): Promise<RunDetails> {
  return invoke<RunDetails>('get_run', { runId });
}

/** 读取一次运行的持久化事件流。 */
export function readRunEvents(runId: string): Promise<RunTraceEvent[]> {
  return invoke<RunTraceEvent[]>('read_run_events', { runId });
}

/** 通过 run_id/artifact_id 读取原始媒体 bytes，前端永远不接收磁盘路径。 */
export function readRunArtifact(
  runId: string,
  artifactId: string,
): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>('read_run_artifact', { runId, artifactId });
}

/** 将 Tauri 抛出的未知值归一化为界面可安全展示的命令错误。 */
export function normalizeCommandError(error: unknown): CommandError {
  if (isCommandError(error)) {
    return error;
  }

  return {
    code: 'unknown_error',
    message: '操作未完成，请稍后重试。',
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
