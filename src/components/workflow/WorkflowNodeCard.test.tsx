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

  it('summarizes an AQL SetValue action on the canvas', () => {
    const node: WorkflowCanvasNode = {
      id: 'action-1',
      kind: 'action',
      position: { x: 0, y: 0 },
      size: { width: 164, height: 52 },
      data: {
        kind: 'action',
        label: '填写记事本',
        action: {
          type: 'set_value',
          target: {
            locator: {
              type: 'query',
              query: { language_version: 1, source: 'document()' },
            },
            backend_preference: 'windows_uia',
          },
          value: 'ArgusFlow',
        },
      },
    };

    render(<WorkflowNodeCard node={node} selected={false} />);

    expect(screen.getByText('填写 · AQL')).toBeVisible();
  });
});
