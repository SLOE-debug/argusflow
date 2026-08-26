import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';
import { WorkflowNodeCard } from './WorkflowNodeCard';

describe('WorkflowNodeCard', () => {
  it('applies the complete light-blue selected presentation', () => {
    const node: WorkflowCanvasNode = {
      id: 'log-1',
      kind: 'log',
      position: { x: 0, y: 0 },
      size: { width: 142, height: 52 },
      data: {
        kind: 'log',
        label: '写入日志',
        message: '记录结果',
      },
    };

    const { container } = render(
      <WorkflowNodeCard
        node={node}
        selected
      />,
    );

    const card = container.querySelector('[data-selected="true"]');
    expect(card).toHaveClass('border-blue-400', 'bg-blue-50');
    expect(screen.getByText('记录结果')).toHaveClass('text-blue-600');
  });

  it('summarizes an AQL SetValue UI operation on the canvas', () => {
    const node: WorkflowCanvasNode = {
      id: 'ui-1',
      kind: 'ui',
      position: { x: 0, y: 0 },
      size: { width: 164, height: 52 },
      data: {
        kind: 'ui',
        label: '填写记事本',
        operation: {
          type: 'set_value',
          target: {
            scope: { type: 'current' },
            locator: {
              type: 'query',
              query: { language_version: 1, source: 'document()' },
            },
            backend_preference: 'windows_uia',
          },
          value: { type: 'literal', value: 'ArgusFlow' },
        },
      },
    };

    render(<WorkflowNodeCard node={node} selected={false} />);

    expect(screen.getByText('填写 · AQL')).toBeVisible();
  });

  it('shows the selected upstream output on a debug node', () => {
    const node: WorkflowCanvasNode = {
      id: 'debug-1',
      kind: 'debug',
      position: { x: 0, y: 0 },
      size: { width: 156, height: 52 },
      data: {
        kind: 'debug',
        label: '输出窗口标题',
        value: {
          type: 'node_output',
          node_id: 'read-title',
          output: 'text',
        },
      },
    };

    render(<WorkflowNodeCard node={node} selected={false} />);

    expect(screen.getByText('read-title.text')).toBeVisible();
  });

  it.each([
    ['pending', '待运行'],
    ['running', '正在运行'],
    ['success', '已完成'],
    ['error', '失败'],
    ['skipped', '未执行'],
  ] as const)('renders the %s runtime state', (runState, label) => {
    const node: WorkflowCanvasNode = {
      id: `log-${runState}`,
      kind: 'log',
      position: { x: 0, y: 0 },
      size: { width: 142, height: 52 },
      data: {
        kind: 'log',
        label: '写入日志',
        message: '记录结果',
        runState,
      },
    };

    const { container } = render(
      <WorkflowNodeCard node={node} selected={false} />,
    );

    expect(container.querySelector(`[data-run-state="${runState}"]`)).not.toBeNull();
    expect(screen.getByText(label)).toBeVisible();
  });
});
