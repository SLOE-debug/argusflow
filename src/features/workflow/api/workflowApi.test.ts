import { describe, expect, it } from 'vitest';

import { normalizeCommandError } from './workflowApi';

/** 覆盖 Tauri 命令错误的结构化保留与未知异常归一化行为。 */
describe('workflow API errors', () => {
  it('preserves structured errors returned by Tauri', () => {
    const error = normalizeCommandError({
      code: 'validation_failed',
      message: '工作流校验失败',
      issues: [],
    });

    expect(error.code).toBe('validation_failed');
    expect(error.message).toBe('工作流校验失败');
  });

  it('normalizes unknown failures for the UI', () => {
    expect(normalizeCommandError(new Error('offline')).message).toBe('操作未完成，请稍后重试。');
  });
});
