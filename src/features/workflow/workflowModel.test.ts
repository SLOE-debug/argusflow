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
      version: 2,
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
        },
      },
    });
    vi.unstubAllGlobals();
  });

  it('uses a browser-to-desktop Baidu news workflow as the default template', () => {
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      DEFAULT_WORKFLOW_NAME,
      DEFAULT_WORKFLOW_INPUTS,
      DEFAULT_WORKFLOW_VARIABLES,
      DEFAULT_WORKFLOW_PERMISSIONS,
      DEFAULT_NODES,
      DEFAULT_EDGES,
    );

    expect(workflow.name).toBe('采集百度热搜并保存到桌面');
    expect(workflow.inputs).toEqual([]);
    expect(DEFAULT_RUN_INPUT_VALUES).toEqual({});
    expect(workflow.nodes.some((node) => node.type_id === 'argus.condition')).toBe(false);
    expect(workflow.nodes.some((node) => node.type_id === 'argus.delay')).toBe(false);
    expect(workflow.edges).toHaveLength(7);
    expect(workflow.edges.every((edge) => edge.branch === null)).toBe(true);
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'baidu_browser_1',
      type_id: 'argus.browser',
      version: 2,
      payload: {
        spec: expect.objectContaining({
          acquire_mode: 'launch_isolated_cdp',
        }),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'navigate_baidu_1',
      type_id: 'argus.browser.operation',
      payload: {
        operation: expect.objectContaining({
          type: 'navigate',
          browser: {
            producer_node_id: 'baidu_browser_1',
            output_name: 'session',
          },
        }),
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'collect_baidu_news_1',
      type_id: 'argus.ui',
      version: 2,
      payload: {
        operation: {
          type: 'extract',
          target: expect.objectContaining({
            scope: {
              type: 'browser',
              resource: {
                producer_node_id: 'baidu_browser_1',
                output_name: 'session',
              },
            },
            backend_policy: {
              allow: ['browser_cdp'],
              deny: [],
              prefer: ['browser_cdp'],
            },
            locator: {
              type: 'query',
              query: {
                language_version: 1,
                source: 'css("#hotsearch-content-wrapper a.title-content")',
              },
            },
          }),
          cardinality: 'many',
          fields: [
            { name: 'title', source: { type: 'text' } },
            { name: 'url', source: { type: 'attribute', name: 'href' } },
          ],
        },
        execution: {
          target_wait: {
            mode: 'bounded',
            timeout_ms: 5_000,
            poll_interval_ms: 100,
          },
        },
      },
    }));
    expect(workflow.nodes).toContainEqual(expect.objectContaining({
      id: 'write_baidu_news_1',
      type_id: 'argus.command',
      payload: {
        operation: expect.objectContaining({
          runner: 'power_shell',
          stdin: {
            type: 'ref',
            source: { type: 'node', node_id: 'format_baidu_news_1' },
            pointer: '/text',
          },
        }),
      },
    }));
    const defaultCommandNode = DEFAULT_NODES.find(
      (node) => node.id === 'write_baidu_news_1',
    );
    if (defaultCommandNode?.data.kind !== 'command') {
      throw new Error('default PowerShell command node is missing');
    }
    expect(defaultCommandNode.data.operation.script).toMatchObject({
      type: 'literal',
      value: expect.stringContaining('\n'),
    });
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
    const exactRoutes = DEFAULT_EDGES.map((edge) => (
      routeEdge(edge, DEFAULT_NODES)?.route ?? null
    ));
    const previewRoutes = DEFAULT_EDGES.map((edge) => (
      previewEdgeRoute(edge, DEFAULT_NODES)?.route ?? null
    ));
    const writeDebugEdgeIndex = DEFAULT_EDGES.findIndex(
      (edge) => edge.id === 'edge_write_debug',
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
    expect(exactRoutes[writeDebugEdgeIndex]).toMatchObject({
      sourceSide: 'left',
      targetSide: 'right',
    });
    expect(exactRoutes[writeDebugEdgeIndex]!.points.length)
      .toBeLessThanOrEqual(6);
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
