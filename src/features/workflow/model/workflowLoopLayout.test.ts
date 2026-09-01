import { describe, expect, it } from 'vitest';

import type { FlowNode } from '../../../flow';
import type { WorkflowNodeData } from './workflowModel';
import {
  resolveWorkflowLoopLayout,
  synchronizeWorkflowLoopContainerSizes,
  type WorkflowLoopDocuments,
} from './workflowLoopLayout';

describe('workflowLoopLayout', () => {
  it('derives both dimensions from the actual child graph bounds', () => {
    const nodes = [
      logNode('left', 100, 50, 100, 50),
      logNode('right', 500, 200, 120, 60),
    ];

    expect(resolveWorkflowLoopLayout(nodes)).toEqual({
      bounds: { x: 100, y: 50, width: 520, height: 210 },
      size: { width: 366, height: 169 },
    });
  });

  it('synchronizes nested loop sizes from the deepest body outward', () => {
    const documents: WorkflowLoopDocuments = {
      root: {
        nodes: [loopNode('outer', 'outer-body', 0, 0)],
        edges: [],
      },
      'outer-body': {
        nodes: [loopNode('inner', 'inner-body', 10, 20)],
        edges: [],
      },
      'inner-body': {
        nodes: [logNode('wide-child', 80, 40, 400, 200)],
        edges: [],
      },
    };

    const synchronized = synchronizeWorkflowLoopContainerSizes(documents);
    expect(synchronized['outer-body']?.nodes[0]?.size).toEqual({
      width: 284,
      height: 162,
    });
    expect(synchronized.root?.nodes[0]?.size).toEqual({
      width: 220,
      height: 137,
    });
    expect(synchronizeWorkflowLoopContainerSizes(synchronized)).toBe(synchronized);
  });
});

/** 建立只含布局测试所需字段的普通节点。 */
function logNode(
  id: string,
  x: number,
  y: number,
  width: number,
  height: number,
): FlowNode<WorkflowNodeData> {
  return {
    id,
    kind: 'log',
    position: { x, y },
    size: { width, height },
    data: {
      kind: 'log',
      label: id,
      message: id,
      outputBindings: {},
    },
  };
}

/** 建立带故意过期尺寸的 While，用于验证深度同步会覆盖持久化值。 */
function loopNode(
  id: string,
  bodyScopeId: string,
  x: number,
  y: number,
): FlowNode<WorkflowNodeData> {
  return {
    id,
    kind: 'loop',
    position: { x, y },
    size: { width: 300, height: 180 },
    data: {
      kind: 'loop',
      label: id,
      bodyScopeId,
      maxIterations: 10,
      timeoutMs: 30_000,
      intervalMs: 500,
      outputBindings: {},
    },
  };
}
