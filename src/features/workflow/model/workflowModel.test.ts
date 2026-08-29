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
  it('maps the empty canvas to the schema v8 Rust contract', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      'Demo',
      DEFAULT_WORKFLOW_INPUTS,
      { enabled: true },
      DEFAULT_WORKFLOW_PERMISSIONS,
      [],
      [],
    );
    expect(workflow.schema_version).toBe(8);
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
      version: 3,
      payload: {
        operation: {
          type: 'click',
          target: {
            scope: { type: 'current' },
            locator: {
              type: 'query',
              query: { language_version: 1 },
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
          postcondition_wait: {
            mode: 'bounded',
            timeout_ms: 5_000,
            poll_interval_ms: 150,
          },
          postcondition: null,
        },
      },
    });
    vi.unstubAllGlobals();
  });

  it('uses a WeChat group search and message workflow as the default template', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      DEFAULT_WORKFLOW_NAME,
      DEFAULT_WORKFLOW_INPUTS,
      DEFAULT_WORKFLOW_VARIABLES,
      DEFAULT_WORKFLOW_PERMISSIONS,
      DEFAULT_NODES,
      DEFAULT_EDGES,
    );

    expect(workflow.name).toBe('搜索微信群并发送测试消息');
    expect(workflow.inputs).toEqual([
      { key: 'group_name', value_type: 'text' },
      { key: 'message', value_type: 'text' },
    ]);
    expect(DEFAULT_RUN_INPUT_VALUES).toEqual({
      group_name: 'ArgusFlow 测试群',
      message: 'ArgusFlow 自动化测试消息',
    });
    expect(workflow.nodes.some((node) => node.type_id === 'argus.condition')).toBe(false);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.delay')).toHaveLength(0);
    expect(workflow.nodes.filter((node) => node.type_id === 'argus.ui')).toHaveLength(9);
    expect(workflow.edges).toHaveLength(11);
    expect(workflow.edges.every((edge) => edge.branch === null)).toBe(true);
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_application_1',
      type_id: 'argus.application',
      payload: {
        spec: expect.objectContaining({
          executable_path: 'C:\\Program Files\\Tencent\\Weixin\\Weixin.exe',
          acquire_policy: 'attach_or_start',
          activation_policy: 'required',
        }),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_open_search_1',
      type_id: 'argus.ui',
      payload: {
        operation: {
          type: 'press_key',
          target: {
            scope: {
              type: 'application',
              resource: {
                producer_node_id: 'wechat_application_1',
                output_name: 'session',
              },
            },
            locator: { type: 'focused' },
            backend_policy: {
              allow: ['send_input'],
              deny: [],
              prefer: ['send_input'],
            },
          },
          chord: {
            key: { type: 'character', value: 'f' },
            modifiers: ['control'],
          },
        },
        execution: {
          target_wait: {
            mode: 'none',
            timeout_ms: 0,
            poll_interval_ms: 0,
          },
          postcondition_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
          postcondition: null,
        },
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_type_group_name_1',
      type_id: 'argus.ui',
      payload: {
        operation: expect.objectContaining({
          type: 'type_text',
          value: {
            type: 'ref',
            source: { type: 'workflow_input', key: 'group_name' },
            pointer: '',
          },
        }),
        execution: expect.any(Object),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_click_group_1',
      type_id: 'argus.ui',
      payload: expect.objectContaining({
        operation: expect.objectContaining({
          type: 'click',
          target: expect.objectContaining({
            locator: {
              type: 'visual',
              query: {
                text: {
                  type: 'ref',
                  source: { type: 'workflow_input', key: 'group_name' },
                  pointer: '',
                },
                exact: true,
                region: {
                  x: 0,
                  y: 0,
                  width: 0.58,
                  height: 0.72,
                },
              },
            },
          }),
        }),
      }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_type_message_1',
      type_id: 'argus.ui',
      payload: {
        operation: expect.objectContaining({
          type: 'type_text',
          value: {
            type: 'ref',
            source: { type: 'workflow_input', key: 'message' },
            pointer: '',
          },
        }),
        execution: expect.any(Object),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_send_message_1',
      type_id: 'argus.ui',
      payload: {
        operation: expect.objectContaining({
          type: 'press_key',
          chord: {
            key: { type: 'enter' },
            modifiers: [],
          },
        }),
        execution: expect.any(Object),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wechat_send_message_1',
      payload: expect.objectContaining({
        execution: expect.objectContaining({
          postcondition_wait: {
            mode: 'bounded',
            timeout_ms: 5_000,
            poll_interval_ms: 150,
          },
        }),
      }),
    }));
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
    const groupNameWaitEdgeIndex = DEFAULT_EDGES.findIndex(
      (edge) => edge.id === 'edge_group_name_find',
    );
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
    expect(exactRoutes[groupNameWaitEdgeIndex]).toMatchObject({
      sourceSide: 'bottom',
      targetSide: 'top',
    });
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
