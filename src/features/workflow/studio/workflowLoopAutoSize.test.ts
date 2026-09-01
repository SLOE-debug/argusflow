import { describe, expect, it } from 'vitest';

import { createFlowStore } from '../../../flow';
import type {
  WorkflowCanvasNode,
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../model/workflowModel';
import { bindWorkflowLoopAutoSize } from './workflowLoopAutoSize';

describe('bindWorkflowLoopAutoSize', () => {
  it('updates the parent document whenever its child graph bounds change', () => {
    const loop = loopNode();
    const child = logNode('child', 0, 0);
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      activeDocumentId: 'body',
      nodes: [child],
      edges: [],
      documents: {
        root: { nodes: [loop], edges: [] },
        body: { nodes: [child], edges: [] },
      },
    });
    const unbind = bindWorkflowLoopAutoSize(store);

    expect(store.getState().documents.root?.nodes[0]?.size).toEqual({
      width: 220,
      height: 120,
    });

    store.getState().setNodes([
      child,
      logNode('far-child', 500, 200),
    ]);

    expect(store.getState().documents.root?.nodes[0]?.size).toEqual({
      width: 449,
      height: 198,
    });
    unbind();
  });
});

/** 建立拥有 body 子作用域的父级 While。 */
function loopNode(): WorkflowCanvasNode {
  return {
    id: 'loop',
    kind: 'loop',
    position: { x: 0, y: 0 },
    size: { width: 1, height: 1 },
    data: {
      kind: 'loop',
      label: 'While',
      bodyScopeId: 'body',
      maxIterations: 10,
      timeoutMs: 30_000,
      intervalMs: 500,
      outputBindings: {},
    },
  };
}

/** 建立固定尺寸的子节点，让测试只关注边界跨度变化。 */
function logNode(id: string, x: number, y: number): WorkflowCanvasNode {
  return {
    id,
    kind: 'log',
    position: { x, y },
    size: { width: 142, height: 52 },
    data: {
      kind: 'log',
      label: id,
      message: id,
      outputBindings: {},
    },
  };
}
