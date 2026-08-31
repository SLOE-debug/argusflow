import { describe, expect, it } from 'vitest';

import { createRegisteredNodeData } from '../model/workflowNodeDefinitions';
import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
} from '../model/workflowModel';
import {
  getWorkflowNodeOutputAvailability,
  isWorkflowNodeOutputAvailable,
} from './workflowSymbolAvailability';
import {
  WECHAT_WORKFLOW_EDGES,
  WECHAT_WORKFLOW_NODES,
} from '../examples/wechat/template';

/** 创建可用于图支配关系测试的最小画布节点。 */
function createNode(
  id: string,
  kind: Parameters<typeof createRegisteredNodeData>[0],
): WorkflowCanvasNode {
  const data = createRegisteredNodeData(kind);
  return {
    id,
    kind: data.kind,
    position: { x: 0, y: 0 },
    size: { width: 100, height: 50 },
    data,
  };
}

/** 创建带可选分支标签的画布边。 */
function createEdge(
  id: string,
  source: string,
  target: string,
  branch: 'true' | 'false' | null = null,
): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: source },
    target: { nodeId: target },
    data: { branch },
  };
}

/** 统一构造可用性查询参数，避免每个场景重复快照字段。 */
function availabilityArgs(
  producerNodeId: string,
  consumerNodeId: string,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  edges: ReadonlyArray<WorkflowCanvasEdge>,
) {
  return { producerNodeId, consumerNodeId, nodes, edges } as const;
}

describe('workflow symbol availability', () => {
  it('accepts a producer that strictly dominates a linear consumer', () => {
    const start = createNode('start', 'start');
    const producer = createNode('producer', 'command');
    const consumer = createNode('consumer', 'debug');
    const nodes = [start, producer, consumer];
    const edges = [
      createEdge('start-producer', start.id, producer.id),
      createEdge('producer-consumer', producer.id, consumer.id),
    ];

    expect(isWorkflowNodeOutputAvailable(
      availabilityArgs(producer.id, consumer.id, nodes, edges),
    )).toBe(true);
  });

  it('rejects a producer that exists only on one branch before a merge', () => {
    const start = createNode('start', 'start');
    const branch = createNode('branch', 'condition');
    const producer = createNode('producer', 'command');
    const other = createNode('other', 'log');
    const merge = createNode('merge', 'debug');
    const nodes = [start, branch, producer, other, merge];
    const edges = [
      createEdge('start-branch', start.id, branch.id),
      createEdge('branch-producer', branch.id, producer.id, 'true'),
      createEdge('branch-other', branch.id, other.id, 'false'),
      createEdge('producer-merge', producer.id, merge.id),
      createEdge('other-merge', other.id, merge.id),
    ];

    expect(getWorkflowNodeOutputAvailability(
      availabilityArgs(producer.id, merge.id, nodes, edges),
    )).toEqual({
      available: false,
      unavailableReason: '并非在所有执行路径上可用',
    });
  });

  it('rejects an orphan producer even when the consumer is reachable', () => {
    const start = createNode('start', 'start');
    const consumer = createNode('consumer', 'debug');
    const orphan = createNode('orphan', 'command');
    const nodes = [start, consumer, orphan];
    const edges = [createEdge('start-consumer', start.id, consumer.id)];

    expect(getWorkflowNodeOutputAvailability(
      availabilityArgs(orphan.id, consumer.id, nodes, edges),
    )).toEqual({
      available: false,
      unavailableReason: '生产节点无法从 Start 到达',
    });
  });

  it('rejects a node output used by the node that produces it', () => {
    const producer = createNode('producer', 'command');
    expect(getWorkflowNodeOutputAvailability({
      producerNodeId: producer.id,
      consumerNodeId: producer.id,
      nodes: [producer],
      edges: [],
    })).toEqual({
      available: false,
      unavailableReason: '生产节点与消费节点相同，当前节点执行前尚未产生该输出',
    });
  });

  it('lists outputs without a consumer constraint while editing workflow-level data', () => {
    const producer = createNode('producer', 'command');
    expect(getWorkflowNodeOutputAvailability({
      producerNodeId: producer.id,
      nodes: [producer],
      edges: [],
    })).toEqual({ available: true });
  });

  it('terminates for the repeated-check branches in the WeChat example', () => {
    expect(getWorkflowNodeOutputAvailability({
      producerNodeId: 'open_wechat',
      consumerNodeId: 'start',
      nodes: WECHAT_WORKFLOW_NODES,
      edges: WECHAT_WORKFLOW_EDGES,
    })).toEqual({
      available: false,
      unavailableReason: '并非在所有执行路径上可用',
    });
  });
});
