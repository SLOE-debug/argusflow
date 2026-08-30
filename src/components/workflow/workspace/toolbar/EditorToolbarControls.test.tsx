import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, type FlowNode } from '../../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../../../features/workflow';
import { EditorPrimaryActions } from './EditorPrimaryActions';
import { EditorToolbarControls } from './EditorToolbarControls';

/** 编辑命令测试使用的最小节点。 */
const node: FlowNode<WorkflowNodeData> = {
  id: 'log-1',
  kind: 'log',
  position: { x: 0, y: 0 },
  size: { width: 168, height: 52 },
  data: { kind: 'log', label: '记录日志', outputBindings: {}, message: '测试日志' },
};

describe('editor title bar controls', () => {
  it('tracks history availability and exposes panel toggles', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({ nodes: [node] });

    render(
      <EditorToolbarControls
        store={store}
        libraryOpen
        dockOpen={false}
        inspectorOpen
        onLibraryOpenChange={vi.fn()}
        onDockOpenChange={vi.fn()}
        onInspectorOpenChange={vi.fn()}
      />,
    );

    const undoButton = screen.getByRole('button', { name: '撤销' });
    expect(undoButton).toBeDisabled();
    expect(undoButton).toHaveClass('size-7');
    expect(undoButton.querySelector('svg')).toHaveClass('size-4', 'shrink-0');
    expect(screen.getByRole('button', { name: '重做' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: '复制' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '删除' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '左侧面板' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: '底部面板' })).toHaveAttribute('aria-pressed', 'false');

    act(() => store.getState().setNodes([]));
    expect(screen.getByRole('button', { name: '撤销' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: '撤销' }));
    expect(store.getState().nodes).toHaveLength(1);
  });

  it('disables backend actions while a workflow is running', () => {
    render(
      <EditorPrimaryActions
        running
        executionEnabled
        onValidate={vi.fn()}
        onRun={vi.fn()}
        onPublish={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: '检查工作流' })).toBeDisabled();
    const runButton = screen.getByRole('button', { name: '运行中…' });
    expect(runButton).toBeDisabled();
    expect(runButton.querySelector('svg')).toHaveClass('size-3.5', 'shrink-0');
    expect(screen.getByRole('button', { name: '运行中…选项' })).toHaveClass(
      '[&>svg]:shrink-0',
    );
    expect(screen.getByRole('button', { name: '发布', hidden: true }).querySelector('svg'))
      .toHaveClass('size-3.5', 'shrink-0');
  });

  it('keeps validation available while runtime execution capabilities are blocked', () => {
    render(
      <EditorPrimaryActions
        running={false}
        executionEnabled={false}
        onValidate={vi.fn()}
        onRun={vi.fn()}
        onPublish={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: '检查工作流' })).toBeEnabled();
    expect(screen.getByRole('button', { name: '运行不可用' })).toBeDisabled();
  });
});
