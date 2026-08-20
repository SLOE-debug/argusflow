import { invoke } from '@tauri-apps/api/core';

import type {
  CommandError,
  RunStarted,
  ValidationReport,
  WorkflowDefinition,
} from './contracts';

/** 请求后端校验工作流结构，并返回可定位到节点或边的问题。 */
export function validateWorkflow(workflow: WorkflowDefinition): Promise<ValidationReport> {
  return invoke<ValidationReport>('validate_workflow', { workflow });
}

/** 请求后端启动工作流，成功时返回本次运行 ID；命令失败时 Promise 会拒绝。 */
export function runWorkflow(workflow: WorkflowDefinition): Promise<RunStarted> {
  return invoke<RunStarted>('run_workflow', { workflow });
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
    typeof candidate.code === 'string' &&
    typeof candidate.message === 'string' &&
    Array.isArray(candidate.issues)
  );
}
