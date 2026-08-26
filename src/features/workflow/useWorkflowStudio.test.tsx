import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

/** 工作流 Hook 测试只观察前端编排，不调用 Tauri IPC。 */
const workflowApiMocks = vi.hoisted(() => ({
  runWorkflow: vi.fn(),
  validateWorkflow: vi.fn(),
}));

vi.mock('./workflowApi', () => ({
  isDesktopRuntime: () => false,
  normalizeCommandError: (error: unknown) => ({
    code: 'unknown_error',
    message: error instanceof Error ? error.message : String(error),
    issues: [],
  }),
  runWorkflow: workflowApiMocks.runWorkflow,
  validateWorkflow: workflowApiMocks.validateWorkflow,
}));

import { useWorkflowStudio } from './useWorkflowStudio';

describe('useWorkflowStudio', () => {
  beforeEach(() => {
    workflowApiMocks.validateWorkflow.mockReset();
    workflowApiMocks.runWorkflow.mockReset();
    workflowApiMocks.validateWorkflow.mockResolvedValue({ valid: true, issues: [] });
    workflowApiMocks.runWorkflow.mockResolvedValue({ run_id: 'run-1' });
  });

  it('adds the node and edge in one undoable transaction', () => {
    const studio = renderHook(() => useWorkflowStudio());
    act(() => studio.result.current.addNode('log', { x: 80, y: 80 }));
    const stateAfterSource = studio.result.current.flowStore.getState();
    const sourceNodeId = stateAfterSource.selectedNodeIds.values().next().value;
    expect(sourceNodeId).toBeDefined();
    const historyCount = stateAfterSource.past.length;
    const nodeCount = stateAfterSource.nodes.length;
    const edgeCount = stateAfterSource.edges.length;

    let added = false;
    act(() => {
      added = studio.result.current.addConnectedNode(
        'delay',
        { x: 240, y: 80 },
        sourceNodeId!,
        'right',
      );
    });

    const connectedState = studio.result.current.flowStore.getState();
    expect(added).toBe(true);
    expect(connectedState.nodes).toHaveLength(nodeCount + 1);
    expect(connectedState.edges).toHaveLength(edgeCount + 1);
    expect(connectedState.past).toHaveLength(historyCount + 1);
    expect(connectedState.edges.at(-1)?.source).toEqual({
      nodeId: sourceNodeId,
      side: 'right',
    });
    expect(connectedState.edges.at(-1)?.target.side).toBe('left');

    act(() => connectedState.undo());
    expect(studio.result.current.flowStore.getState().nodes).toHaveLength(nodeCount);
    expect(studio.result.current.flowStore.getState().edges).toHaveLength(edgeCount);
  });

  it('binds a UI node directly connected from an Application to its session', () => {
    const studio = renderHook(() => useWorkflowStudio());
    act(() => studio.result.current.addNode('application', { x: 80, y: 280 }));
    const applicationId = studio.result.current.flowStore
      .getState()
      .selectedNodeIds
      .values()
      .next()
      .value;

    act(() => {
      studio.result.current.addConnectedNode(
        'ui',
        { x: 280, y: 280 },
        applicationId!,
        'right',
      );
    });

    const selectedId = studio.result.current.flowStore
      .getState()
      .selectedNodeIds
      .values()
      .next()
      .value;
    const uiNode = studio.result.current.flowStore
      .getState()
      .nodes
      .find((node) => node.id === selectedId);
    expect(uiNode?.data.kind).toBe('ui');
    if (uiNode?.data.kind !== 'ui') throw new Error('expected UI node');
    expect(uiNode.data.operation.target.scope).toEqual({
      type: 'application',
      resource: {
        producer_node_id: applicationId,
        output_name: 'session',
      },
    });
  });

  it('marks every node pending before dispatching a validated run', async () => {
    const studio = renderHook(() => useWorkflowStudio());

    await act(async () => {
      await studio.result.current.run();
    });

    expect(workflowApiMocks.validateWorkflow).toHaveBeenCalledOnce();
    expect(workflowApiMocks.runWorkflow).toHaveBeenCalledOnce();
    expect(studio.result.current.flowStore.getState().nodes.every(
      (node) => node.data.runState === 'pending',
    )).toBe(true);
    expect(studio.result.current.running).toBe(true);
  });
});
