import { describe, expect, it, vi } from 'vitest';

import { previewEdgeRoute, routeEdge } from '../../../flow';
import type { FlowNode, FlowPoint } from '../../../flow';
import {
  DEFAULT_EDGES,
  DEFAULT_NODES,
  DEFAULT_RUN_INPUT_VALUES,
  DEFAULT_WORKFLOW_NAME,
  DEFAULT_WORKFLOW_INPUTS,
  DEFAULT_WORKFLOW_PERMISSIONS,
  DEFAULT_WORKFLOW_VARIABLES,
} from './defaultWorkflowTemplate';
import {
  WORKFLOW_NODE_SIZES,
  applyExecutionEventToNodes,
  canConnect,
  createEdge,
  createNode,
  toWorkflowDefinition,
  type EditableNodeKind,
  type NodeRunState,
} from './workflowModel';

/** 创建状态转换测试所需的最小执行事件。 */
function createExecutionEvent(
  kind: import('./contracts').ExecutionEventKind,
  nodeId: string | null = null,
): import('./contracts').ExecutionEvent {
  return {
    run_id: 'run-1',
    workflow_id: 'workflow-1',
    sequence: 1,
    node_id: nodeId,
    edge_id: null,
    kind,
    message: null,
    payload: null,
  };
}

describe('workflow model', () => {
  it('maps the empty canvas to the schema v9 Rust contract', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      'Demo',
      DEFAULT_WORKFLOW_INPUTS,
      { enabled: true },
      DEFAULT_WORKFLOW_PERMISSIONS,
      [],
      [],
    );
    expect(workflow.schema_version).toBe(9);
    expect(workflow.variables).toEqual({ enabled: true });
    expect(workflow.nodes).toEqual([]);
  });

  it('creates a condition node with a JSON predicate', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const node = createNode('condition', { x: 20, y: 40 });
    expect(node.id).toBe('condition-generated-id');
    if (node.data.kind !== 'condition') throw new Error('expected condition data');
    expect(node.data.operator).toBe('equal');
    expect(node.position).toEqual({ x: 20, y: 40 });
    vi.unstubAllGlobals();
  });

  it('serializes a UI node into the semantic operation contract', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const action = createNode('ui', { x: 20, y: 40 });
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      'UI automation',
      DEFAULT_WORKFLOW_INPUTS,
      {},
      DEFAULT_WORKFLOW_PERMISSIONS,
      [action],
      [],
    );

    expect(workflow.nodes[0]).toMatchObject({
      type_id: 'argus.ui',
      version: 5,
      payload: {
        operation: {
          type: 'click',
          target: {
            scope: { type: 'current' },
            locator: {
              type: 'query',
              query: { language_version: 3, bindings: {} },
            },
            backend_policy: { allow: [], deny: [], prefer: [] },
          },
        },
        execution: {
          target_wait: {
            mode: 'bounded',
            timeout_ms: 5_000,
            poll_interval_ms: 100,
          },
        },
      },
    });
    vi.unstubAllGlobals();
  });

  it('uses the expanded WeChat send-message workflow as the default template', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      DEFAULT_WORKFLOW_NAME,
      DEFAULT_WORKFLOW_INPUTS,
      DEFAULT_WORKFLOW_VARIABLES,
      DEFAULT_WORKFLOW_PERMISSIONS,
      DEFAULT_NODES,
      DEFAULT_EDGES,
    );

    expect(workflow.name).toBe('微信：搜索联系人并发送消息');
    expect(workflow.inputs).toEqual([
      { key: '联系人', value_type: 'text' },
      { key: '消息内容', value_type: 'text' },
    ]);
    expect(DEFAULT_RUN_INPUT_VALUES).toEqual({
      联系人: '文件传输助手',
      消息内容: 'ArgusFlow 测试消息',
    });
    expect(workflow.permissions.allow).toContain('process.application.launch');
    expect(workflow.nodes).toHaveLength(19);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.ui')).toHaveLength(6);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.observe')).toHaveLength(3);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.loop')).toHaveLength(3);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.fail')).toHaveLength(3);
    expect(workflow.edges).toHaveLength(24);
    expect(workflow.nodes[0]?.id).toBe('start');
    expect(workflow.nodes.at(-1)?.id).toBe('end');
  });

  it('uses one compact size contract for workflow models', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const kinds = Object.keys(WORKFLOW_NODE_SIZES) as EditableNodeKind[];

    for (const kind of kinds) {
      expect(createNode(kind).size).toEqual(WORKFLOW_NODE_SIZES[kind]);
    }

    vi.unstubAllGlobals();
  });

  it('rounds node positions to integer world coordinates', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });

    const node = createNode('log', { x: 32.593, y: 111.6 });

    expect(node.position).toEqual({ x: 33, y: 112 });
    vi.unstubAllGlobals();
  });

  it('assigns true and false branches and rejects a third condition edge', () => {
    let counter = 0;
    vi.stubGlobal('crypto', { randomUUID: () => String(++counter) });
    const condition = createNode('condition');
    const firstTarget = createNode('log');
    const secondTarget = createNode('delay');
    const thirdTarget = createNode('end');
    const nodes = [condition, firstTarget, secondTarget, thirdTarget];
    const first = createEdge(condition.id, firstTarget.id, nodes, []);
    const second = createEdge(condition.id, secondTarget.id, nodes, [first]);
    expect([first.data.branch, second.data.branch]).toEqual(['true', 'false']);
    expect(canConnect(nodes, [first, second], condition.id, thirdTarget.id)).toBe(false);
    vi.unstubAllGlobals();
  });

  it('keeps every default workflow connection visible', () => {
    const exactRoutes = DEFAULT_EDGES.map((edge) => (
      routeEdge(edge, DEFAULT_NODES)?.route ?? null
    ));
    const previewRoutes = DEFAULT_EDGES.map((edge) => (
      previewEdgeRoute(edge, DEFAULT_NODES)?.route ?? null
    ));
    const routeGroups = [
      ['exact', exactRoutes],
      ['preview', previewRoutes],
    ] as const;
    const nodeCrossings = routeGroups.flatMap(([routeKind, routes]) => (
      routes.flatMap((route, routeIndex) => {
        if (!route) {
          return [`${routeKind}:${DEFAULT_EDGES[routeIndex].id}:missing`];
        }
        return DEFAULT_NODES.flatMap((node) => (
          route.points.slice(1).some((point, pointIndex) => (
            segmentCrossesNodeInterior(route.points[pointIndex], point, node)
          ))
            ? [`${routeKind}:${DEFAULT_EDGES[routeIndex].id}:${node.id}`]
            : []
        ));
      })
    ));

    expect(exactRoutes.every((route) => route !== null)).toBe(true);
    expect(previewRoutes.every((route) => route !== null)).toBe(true);
    expect(nodeCrossings).toEqual([]);
  });

  it('applies the complete execution state lifecycle', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'node-id' });
    let nodes = [createNode('log')];

    nodes = applyExecutionEventToNodes(
      nodes,
      createExecutionEvent('workflow_started'),
    );
    expect(nodes[0]?.data.runState).toBe('pending');

    nodes = applyExecutionEventToNodes(
      nodes,
      createExecutionEvent('node_started', nodes[0]!.id),
    );
    expect(nodes[0]?.data.runState).toBe('running');

    nodes = applyExecutionEventToNodes(
      nodes,
      createExecutionEvent('node_succeeded', nodes[0]!.id),
    );
    expect(nodes[0]?.data.runState).toBe('success');
    vi.unstubAllGlobals();
  });

  it('marks pending branches as skipped when the workflow finishes', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'node-id' });
    const node = createNode('condition');
    const states: ReadonlyArray<NodeRunState> = ['pending', 'success', 'error'];
    const nodes = states.map((runState, index) => ({
      ...node,
      id: `node-${index}`,
      data: { ...node.data, runState },
    }));

    const completed = applyExecutionEventToNodes(
      nodes,
      createExecutionEvent('workflow_completed'),
    );

    expect(completed.map((item) => item.data.runState)).toEqual([
      'skipped',
      'success',
      'error',
    ]);
    vi.unstubAllGlobals();
  });

  it('marks an unfinished running node as failed when the workflow fails', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'node-id' });
    const node = createNode('log');
    const states: ReadonlyArray<NodeRunState> = [
      'running',
      'pending',
      'success',
      'error',
    ];
    const nodes = states.map((runState, index) => ({
      ...node,
      id: `node-${index}`,
      data: { ...node.data, runState },
    }));

    const failed = applyExecutionEventToNodes(
      nodes,
      createExecutionEvent('workflow_failed'),
    );

    expect(failed.map((item) => item.data.runState)).toEqual([
      'error',
      'skipped',
      'success',
      'error',
    ]);
    vi.unstubAllGlobals();
  });
});

/** 判断正交线段是否进入节点内部；仅接触端口所在边界不视为穿透。 */
function segmentCrossesNodeInterior(
  start: FlowPoint,
  end: FlowPoint,
  node: FlowNode,
): boolean {
  const left = node.position.x;
  const right = left + node.size.width;
  const top = node.position.y;
  const bottom = top + node.size.height;
  if (start.x === end.x) {
    const segmentTop = Math.min(start.y, end.y);
    const segmentBottom = Math.max(start.y, end.y);
    return start.x > left
      && start.x < right
      && Math.max(segmentTop, top) < Math.min(segmentBottom, bottom);
  }
  if (start.y === end.y) {
    const segmentLeft = Math.min(start.x, end.x);
    const segmentRight = Math.max(start.x, end.x);
    return start.y > top
      && start.y < bottom
      && Math.max(segmentLeft, left) < Math.min(segmentRight, right);
  }
  return true;
}
