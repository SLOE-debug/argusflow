import { describe, expect, it } from 'vitest';

import { FlowRouteEngine } from './routeEngine';
import type { FlowEdge, FlowNode } from '../types';

/** 构造 25×20 的规则节点阵列，用于观察大文档增量失效规模。 */
function createPerformanceNodes(): FlowNode[] {
  return Array.from({ length: 500 }, (_, index) => ({
    id: `node-${index}`,
    kind: 'test',
    position: {
      x: index % 25 * 160,
      y: Math.floor(index / 25) * 120,
    },
    size: { width: 80, height: 60 },
    data: null,
  }));
}

/** 构造 480 条水平边和 220 条垂直边，共 700 条明确端口边。 */
function createPerformanceEdges(): FlowEdge[] {
  const edges: FlowEdge[] = [];
  for (let row = 0; row < 20; row += 1) {
    for (let column = 0; column < 24; column += 1) {
      const sourceIndex = row * 25 + column;
      edges.push({
        id: `horizontal-${sourceIndex}`,
        source: { nodeId: `node-${sourceIndex}`, side: 'right' },
        target: { nodeId: `node-${sourceIndex + 1}`, side: 'left' },
        data: null,
      });
    }
  }
  for (let sourceIndex = 0; sourceIndex < 220; sourceIndex += 1) {
    edges.push({
      id: `vertical-${sourceIndex}`,
      source: { nodeId: `node-${sourceIndex}`, side: 'bottom' },
      target: { nodeId: `node-${sourceIndex + 25}`, side: 'top' },
      data: null,
    });
  }
  return edges;
}

describe('FlowRouteEngine large document metrics', () => {
  it('keeps a single-node drag local in a 500-node 700-edge document', () => {
    const engine = new FlowRouteEngine();
    const nodes = createPerformanceNodes();
    const edges = createPerformanceEdges();
    engine.update({ nodes, edges, interaction: { kind: 'idle' } });

    const movedNodes = nodes.map((node) => node.id === 'node-0'
      ? { ...node, position: { x: 5, y: 5 } }
      : node);
    const moved = engine.update({
      nodes: movedNodes,
      edges,
      interaction: {
        kind: 'node-drag',
        nodeIds: ['node-0'],
        interactionId: 1,
      },
    });

    expect(moved.routes).toHaveLength(700);
    expect(moved.stats.dirtyEdgeCount).toBeLessThan(10);
    expect(moved.stats.fastRepairHits).toBeGreaterThan(0);
    expect(Number.isFinite(moved.stats.routeTimeMs)).toBe(true);
  });
});
