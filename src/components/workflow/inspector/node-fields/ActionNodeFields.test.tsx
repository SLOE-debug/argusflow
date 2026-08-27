import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { UiOperation } from '../../../../features/workflow';
import { ActionNodeFields } from './ActionNodeFields';

describe('ActionNodeFields', () => {
  it('opens the AQL document in Workspace without mounting an Inspector editor', () => {
    const onChange = vi.fn();
    const onOpenEditor = vi.fn();
    const operation: UiOperation = {
      type: 'click',
      target: {
        scope: { type: 'current' },
        locator: {
          type: 'query',
          query: { language_version: 1, source: 'button(name = "保存")' },
        },
        backend_policy: {
          allow: ['windows_uia'],
          deny: [],
          prefer: ['windows_uia'],
        },
      },
    };

    render(
      <ActionNodeFields
        nodeId="ui-save"
        operation={operation}
        execution={{
          target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
        }}
        onChange={onChange}
        onExecutionChange={vi.fn()}
        onOpenEditor={onOpenEditor}
      />,
    );

    expect(screen.queryByRole('textbox', { name: 'AQL 查找条件' })).not.toBeInTheDocument();
    expect(screen.getByText('button(name = "保存")')).toBeVisible();
    const aqlHeading = screen.getByRole('heading', { name: '查找条件' });
    const advancedSettings = screen.getByText('更多设置');
    expect(aqlHeading.compareDocumentPosition(advancedSettings)
      & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(screen.getByText('请在 ArgusFlow 桌面应用中检查运行环境。')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '编辑条件' }));
    expect(onOpenEditor).toHaveBeenCalledWith({ type: 'aql', nodeId: 'ui-save' });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('edits the node-owned target wait policy without adding another selector', () => {
    const onExecutionChange = vi.fn();
    const operation: UiOperation = {
      type: 'click',
      target: {
        scope: { type: 'current' },
        locator: {
          type: 'query',
          query: { language_version: 1, source: 'button(name = "继续")' },
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
      },
    };

    render(
      <ActionNodeFields
        nodeId="ui-continue"
        operation={operation}
        execution={{
          target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
        }}
        onChange={vi.fn()}
        onExecutionChange={onExecutionChange}
        onOpenEditor={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByText('等待目标'));
    fireEvent.change(screen.getByRole('spinbutton', { name: '目标等待超时时间' }), {
      target: { value: '8000' },
    });
    expect(onExecutionChange).toHaveBeenCalledWith({
      target_wait: { mode: 'bounded', timeout_ms: 8_000, poll_interval_ms: 100 },
    });

    fireEvent.click(screen.getByRole('checkbox', { name: '找不到目标时自动等待' }));
    expect(onExecutionChange).toHaveBeenLastCalledWith({
      target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    });
    expect(screen.getAllByText('button(name = "继续")')).toHaveLength(1);
  });
});
