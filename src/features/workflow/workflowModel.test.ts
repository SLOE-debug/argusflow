import { describe, expect, it, vi } from 'vitest';

import { workflowNodeRegistry } from '../../components/workflow/WorkflowNodeCard';
import { previewEdgeRoute, routeEdge } from '../../flow/routing';
import type { FlowNode, FlowPoint } from '../../flow/types';
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
  it('maps the empty canvas to the schema v5 Rust contract', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      'Demo',
      DEFAULT_WORKFLOW_INPUTS,
      { enabled: true },
      DEFAULT_WORKFLOW_PERMISSIONS,
      [],
      [],
    );
    expect(workflow.schema_version).toBe(5);
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
      type: 'ui',
      operation: {
        type: 'click',
        target: {
          scope: { type: 'current' },
          locator: {
            type: 'query',
            query: { language_version: 1 },
          },
          backend_preference: 'auto',
        },
      },
    });
    vi.unstubAllGlobals();
  });

  it('uses a multi-step Notepad++ UIA workflow as the default template', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      DEFAULT_WORKFLOW_NAME,
      DEFAULT_WORKFLOW_INPUTS,
      DEFAULT_WORKFLOW_VARIABLES,
      DEFAULT_WORKFLOW_PERMISSIONS,
      DEFAULT_NODES,
      DEFAULT_EDGES,
    );

    expect(workflow.name).toBe('用 UIA 驱动 Notepad++ 查找');
    expect(workflow.inputs).toEqual([
      { key: 'search_text', value_type: 'text' },
    ]);
    expect(DEFAULT_RUN_INPUT_VALUES).toEqual({
      search_text: 'UIA',
    });
    expect(workflow.nodes.some((node) => node.type === 'condition')).toBe(false);
    expect(workflow.edges).toHaveLength(13);
    expect(workflow.edges.every((edge) => edge.branch === null)).toBe(true);
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'wait_notepadpp_ready_1',
      type: 'delay',
      milliseconds: 1_000,
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      type: 'application',
      spec: expect.objectContaining({ acquire_policy: 'attach_or_start' }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'open_search_menu_1',
      type: 'ui',
      operation: expect.objectContaining({
        type: 'click',
        target: expect.objectContaining({
          locator: expect.objectContaining({
            query: expect.objectContaining({
              source: 'menu_item(name = "搜索(S)")',
            }),
          }),
        }),
      }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'open_find_dialog_1',
      type: 'ui',
      operation: expect.objectContaining({
        type: 'click',
        target: expect.objectContaining({
          locator: expect.objectContaining({
            query: expect.objectContaining({
              source: 'menu_item(name starts_with "查找(F)...")',
            }),
          }),
        }),
      }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'set_find_value_1',
      type: 'ui',
      operation: expect.objectContaining({
        type: 'set_value',
        value: { type: 'workflow_input', key: 'search_text' },
      }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'count_matches_1',
      type: 'ui',
      operation: expect.objectContaining({ type: 'click' }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'read_search_value_1',
      type: 'ui',
      operation: expect.objectContaining({
        type: 'get_value',
        target: expect.objectContaining({
          locator: expect.objectContaining({
            query: expect.objectContaining({
              source: 'dialog(name = "查找") >> textbox(name = "查找目标(F) :")',
            }),
          }),
        }),
      }),
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      type: 'debug',
      value: {
        type: 'node_output',
        node_id: 'read_search_value_1',
        output: 'value',
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'close_find_dialog_1',
      type: 'ui',
      operation: expect.objectContaining({
        type: 'click',
        target: expect.objectContaining({
          backend_preference: 'windows_uia',
          locator: expect.objectContaining({
            type: 'query',
            query: expect.objectContaining({
              source: 'dialog(name = "查找") >> button(name = "取消")',
            }),
          }),
        }),
      }),
    }));
  });

  it('uses one compact size contract for models and renderers', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const kinds = Object.keys(WORKFLOW_NODE_SIZES) as EditableNodeKind[];

    for (const kind of kinds) {
      expect(createNode(kind).size).toEqual(WORKFLOW_NODE_SIZES[kind]);
      expect(workflowNodeRegistry[kind].defaultSize).toEqual(WORKFLOW_NODE_SIZES[kind]);
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
    const exactRoutes = DEFAULT_EDGES.map((edge) => routeEdge(edge, DEFAULT_NODES));
    const previewRoutes = DEFAULT_EDGES.map((edge) => (
      previewEdgeRoute(edge, DEFAULT_NODES)
    ));
    const waitReadEdgeIndex = DEFAULT_EDGES.findIndex(
      (edge) => edge.id === 'edge_wait_read',
    );
    const waitSetEdgeIndex = DEFAULT_EDGES.findIndex(
      (edge) => edge.id === 'edge_wait_set',
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
    expect(exactRoutes[waitReadEdgeIndex]).toMatchObject({
      sourceSide: 'bottom',
      targetSide: 'top',
    });
    expect(exactRoutes[waitSetEdgeIndex]).toMatchObject({
      sourceSide: 'left',
      targetSide: 'right',
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
