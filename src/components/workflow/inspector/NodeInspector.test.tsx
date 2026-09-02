import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore } from '../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../../features/workflow';
import { DEFAULT_WORKFLOW_PERMISSIONS } from '../../../features/workflow';
import { NodeInspector } from './NodeInspector';

describe('NodeInspector', () => {
  it('uses one property panel and follows the current selection', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      nodes: [{
        id: 'log-1',
        kind: 'log',
        position: { x: 10, y: 20 },
        size: { width: 142, height: 52 },
        data: { kind: 'log', label: '日志', outputBindings: {}, message: '测试' },
      }],
    });
    render(
      <NodeInspector
        store={store}
        workflowName="测试流程"
        permissions={DEFAULT_WORKFLOW_PERMISSIONS}
        onNameChange={vi.fn()}
        onCollapse={vi.fn()}
        onPermissionsChange={vi.fn()}
        onUpdateNode={vi.fn()}
        onUpdateEdgeBranch={vi.fn()}
        onOpenStructuredEditor={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByRole('heading', { name: '属性' })).toBeVisible();
    expect(screen.getByDisplayValue('测试流程')).toBeVisible();
    expect(screen.queryByRole('button', { name: '流程设置' })).not.toBeInTheDocument();

    act(() => store.getState().selectNodes(['log-1']));
    expect(screen.getByText('节点', { selector: 'span' })).toBeVisible();
    expect(screen.getByText('开发者信息')).toBeVisible();
    expect(screen.getByLabelText('内部编号')).toHaveTextContent('log-1');
    expect(document.querySelector('details')).toBeNull();
    expect(screen.queryByDisplayValue('测试流程')).not.toBeInTheDocument();
  });

  it('enters a non-editable missing state when the selected node no longer exists', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    act(() => store.getState().selectNodes(['deleted-node']));

    render(
      <NodeInspector
        store={store}
        workflowName="测试流程"
        permissions={DEFAULT_WORKFLOW_PERMISSIONS}
        onNameChange={vi.fn()}
        onCollapse={vi.fn()}
        onPermissionsChange={vi.fn()}
        onUpdateNode={vi.fn()}
        onUpdateEdgeBranch={vi.fn()}
        onOpenStructuredEditor={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText('此节点已被删除或无法找到')).toBeVisible();
    expect(screen.queryByDisplayValue('测试流程')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.getByDisplayValue('测试流程')).toBeVisible();
  });

  it('keeps action property edits in the shared undo and redo history', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      nodes: [createActionNode()],
    });
    act(() => store.getState().selectNodes(['ui-1']));
    /** 测试使用与 Studio 相同的单一文档事务写回路径。 */
    const updateNode = (updater: (current: WorkflowNodeData) => WorkflowNodeData) => {
      store.getState().transact((document) => ({
        ...document,
        nodes: document.nodes.map((node) => node.id === 'ui-1'
          ? { ...node, data: updater(node.data) }
          : node),
      }), 'node-fields:ui-1');
    };

    render(
      <NodeInspector
        store={store}
        workflowName="测试流程"
        permissions={DEFAULT_WORKFLOW_PERMISSIONS}
        onNameChange={vi.fn()}
        onCollapse={vi.fn()}
        onPermissionsChange={vi.fn()}
        onUpdateNode={updateNode}
        onUpdateEdgeBranch={vi.fn()}
        onOpenStructuredEditor={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole('textbox', { name: '文字' }), {
      target: { value: '继续' },
    });
    expect(actionQuerySource(store.getState().nodes[0]?.data)).toBe('text(name = "继续")');

    act(() => store.getState().undo());
    expect(actionQuerySource(store.getState().nodes[0]?.data)).toBe('text(name = "确定")');

    act(() => store.getState().redo());
    expect(actionQuerySource(store.getState().nodes[0]?.data)).toBe('text(name = "继续")');
  });
});

/** 创建目标内容可直接编辑的 UI 节点。 */
function createActionNode() {
  return {
    id: 'ui-1',
    kind: 'ui' as const,
    position: { x: 10, y: 20 },
    size: { width: 164, height: 52 },
    data: {
      kind: 'ui' as const,
      label: '单击确定',
      outputBindings: {},
      operation: {
        type: 'click' as const,
        target: {
          scope: { type: 'current' as const },
          locator: {
            type: 'query' as const,
            query: { language_version: 3 as const, source: 'text(name = "确定")', bindings: {} },
          },
          backend_policy: { allow: [], deny: [], prefer: [] },
        },
      },
      execution: {
        target_wait: { mode: 'bounded' as const, timeout_ms: 5_000, poll_interval_ms: 100 },
      },
    },
  };
}

/** 从测试节点读取当前 AQL；非查询节点返回空值以暴露意外类型切换。 */
function actionQuerySource(data: WorkflowNodeData | undefined): string | null {
  return data?.kind === 'ui' && data.operation.target.locator.type === 'query'
    ? data.operation.target.locator.query.source
    : null;
}
