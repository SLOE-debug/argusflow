import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, type FlowNode } from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import { EditorPrimaryActions } from './EditorPrimaryActions';
import { EditorToolbarControls } from './EditorToolbarControls';

/** 编辑命令测试使用的最小节点。 */
const node: FlowNode<WorkflowNodeData> = {
  id: 'log-1',
  kind: 'log',
  position: { x: 0, y: 0 },
  size: { width: 168, height: 52 },
  data: { kind: 'log', label: '日志', message: '测试日志' },
};

describe('editor title bar controls', () => {
  it('tracks history, selection and clipboard availability', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({ nodes: [node] });

    render(
      <EditorToolbarControls
        store={store}
        libraryOpen
        inspectorOpen
        consoleOpen={false}
        onToggleLibrary={vi.fn()}
        onToggleInspector={vi.fn()}
        onToggleConsole={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: '撤销' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '复制' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '粘贴' })).toBeDisabled();

    act(() => store.getState().selectNodes([node.id]));
    fireEvent.click(screen.getByRole('button', { name: '复制' }));
    expect(store.getState().clipboard?.nodes).toHaveLength(1);
    expect(screen.getByRole('button', { name: '粘贴' })).toBeEnabled();

    act(() => store.getState().setNodes([]));
    expect(screen.getByRole('button', { name: '撤销' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: '撤销' }));
    expect(store.getState().nodes).toHaveLength(1);
  });

  it('disables backend actions while a workflow is running', () => {
    render(
      <EditorPrimaryActions
        running
        onValidate={vi.fn()}
        onRun={vi.fn()}
        onPublish={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: '校验' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '运行中…' })).toBeDisabled();
  });
});
