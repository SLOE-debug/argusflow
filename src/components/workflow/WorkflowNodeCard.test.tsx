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
});
