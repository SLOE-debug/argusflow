import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { useWorkflowStudio } from './useWorkflowStudio';

describe('useWorkflowStudio connected node creation', () => {
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
});
