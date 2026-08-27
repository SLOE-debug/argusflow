import { describe, expect, it } from 'vitest';

import { rectsIntersect } from '../geometry/geometry';
import { FlowRouteEngine } from './routeEngine';
import type { FlowEdge, FlowNode, FlowPoint, FlowRect } from '../types';

/** 创建固定尺寸的路由测试节点。 */
function createNode(id: string, x: number, y: number): FlowNode {
  return {
    id,
    kind: 'test',
    position: { x, y },
    size: { width: 80, height: 60 },
    data: null,
  };
}

/** 判断路线任一线段是否穿过给定矩形。 */
function routeIntersects(
  points: ReadonlyArray<FlowPoint>,
  rect: FlowRect,
): boolean {
  return points.slice(1).some((point, index) => {
    const previous = points[index];
    return rectsIntersect({
      x: Math.min(previous.x, point.x),
      y: Math.min(previous.y, point.y),
      width: Math.abs(previous.x - point.x),
      height: Math.abs(previous.y - point.y),
    }, rect);
  });
}

describe('FlowRouteEngine', () => {
  it('reroutes a non-incident edge hit by a moved obstacle swept area', () => {
    const engine = new FlowRouteEngine();
    const edges: FlowEdge[] = [{
      id: 'edge',
      source: { nodeId: 'a', side: 'right' },
      target: { nodeId: 'b', side: 'left' },
      data: null,
    }];
    const initialNodes = [
      createNode('a', 0, 0),
      createNode('b', 400, 0),
      createNode('moving', 180, 140),
    ];
    const initial = engine.update({
      nodes: initialNodes,
      edges,
      interaction: { kind: 'idle' },
    });
    expect(initial.routes).toHaveLength(1);

    const movedNodes = initialNodes.map((node) => node.id === 'moving'
      ? { ...node, position: { x: 180, y: 0 } }
      : node);
    const moved = engine.update({
      nodes: movedNodes,
      edges,
      interaction: {
        kind: 'node-drag',
        nodeIds: ['moving'],
        interactionId: 1,
      },
    });
    const movingRect = {
      ...movedNodes[2].position,
      ...movedNodes[2].size,
    };
    expect(moved.dirtyEdgeIds).toContain('edge');
    expect(routeIntersects(moved.routes[0].points, movingRect)).toBe(false);
  });

  it('keeps an emergency route visible for pathological overlap', () => {
    const engine = new FlowRouteEngine();
    const nodes = [
      createNode('a', 0, 0),
      createNode('b', 20, 10),
    ];
    const edges: FlowEdge[] = [{
      id: 'edge',
      source: { nodeId: 'a', side: 'right' },
      target: { nodeId: 'b', side: 'left' },
      data: null,
    }];
    const output = engine.update({
      nodes,
      edges,
      interaction: { kind: 'idle' },
    });
    expect(output.routes).toHaveLength(1);
    expect(output.routes[0].path).not.toBe('');
  });

  it('repairs incident routes from the previous frame before graph search', () => {
    const engine = new FlowRouteEngine();
    const edges: FlowEdge[] = [{
      id: 'edge',
      source: { nodeId: 'a', side: 'right' },
      target: { nodeId: 'b', side: 'left' },
      data: null,
    }];
    const initialNodes = [createNode('a', 0, 0), createNode('b', 400, 0)];
    engine.update({
      nodes: initialNodes,
      edges,
      interaction: { kind: 'idle' },
    });
    const movedNodes = [createNode('a', 20, 20), initialNodes[1]];
    const moved = engine.update({
      nodes: movedNodes,
      edges,
      interaction: {
        kind: 'node-drag',
        nodeIds: ['a'],
        interactionId: 1,
      },
    });

    expect(moved.stats.fastRepairHits).toBe(1);
    expect(moved.stats.localGraphVertices).toBe(0);
  });
});
