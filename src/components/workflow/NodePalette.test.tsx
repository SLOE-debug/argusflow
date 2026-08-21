import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, FLOW_NODE_KIND_DRAG_TYPE } from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import { NodePalette } from './NodePalette';

describe('NodePalette', () => {
  it('publishes enabled node kinds through native drag data', () => {
    const setData = vi.fn();
    const dataTransfer = { effectAllowed: 'none', setData };

    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    render(<NodePalette store={store} />);

    const triggerNode = screen.getByRole('button', { name: '手动触发' });
    expect(triggerNode).toHaveAttribute('draggable', 'true');

    fireEvent.dragStart(triggerNode, { dataTransfer });

    expect(dataTransfer.effectAllowed).toBe('copy');
    expect(setData).toHaveBeenCalledWith(FLOW_NODE_KIND_DRAG_TYPE, 'start');
  });

  it('collapses and expands node groups', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    render(<NodePalette store={store} />);

    const inputGroup = screen.getByRole('button', { name: /输入/ });
    fireEvent.click(inputGroup);
    expect(screen.queryByRole('button', { name: '手动触发' })).not.toBeInTheDocument();
    expect(inputGroup).toHaveAttribute('aria-expanded', 'false');

    fireEvent.click(inputGroup);
    expect(screen.getByRole('button', { name: '手动触发' })).toBeInTheDocument();
    expect(inputGroup).toHaveAttribute('aria-expanded', 'true');
  });
});
