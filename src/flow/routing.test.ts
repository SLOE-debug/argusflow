import { describe, expect, it } from 'vitest';

import { previewEdgeRoute, routeEdge } from './routing';
import type { FlowEdge, FlowNode } from './types';

describe('orthogonal router', () => {
  it('routes around an inflated obstacle', () => {
    const nodes: FlowNode[] = [
      { id: 'a', kind: 'test', position: { x: 0, y: 0 }, size: { width: 80, height: 60 }, data: null },
      { id: 'block', kind: 'test', position: { x: 130, y: -10 }, size: { width: 80, height: 80 }, data: null },
      { id: 'b', kind: 'test', position: { x: 280, y: 0 }, size: { width: 80, height: 60 }, data: null },
    ];
    const edge: FlowEdge = { id: 'edge', source: { nodeId: 'a' }, target: { nodeId: 'b' }, data: null };
    const route = routeEdge(edge, nodes);
    expect(route).not.toBeNull();
    expect(route!.points.length).toBeGreaterThan(2);
    expect(route!.path).toContain('Q');
  });

  it('keeps an orthogonal preview attached to moving endpoints', () => {
    const nodes: FlowNode[] = [
      { id: 'a', kind: 'test', position: { x: 0, y: 0 }, size: { width: 80, height: 60 }, data: null },
      { id: 'b', kind: 'test', position: { x: 280, y: 0 }, size: { width: 80, height: 60 }, data: null },
    ];
    const edge: FlowEdge = {
      id: 'edge',
      source: { nodeId: 'a', side: 'right' },
      target: { nodeId: 'b', side: 'left' },
      data: null,
    };
    const exact = routeEdge(edge, nodes)!;
    const movedNodes = nodes.map((node) => node.id === 'a'
      ? { ...node, position: { x: 40, y: 80 } }
      : node);

    const preview = previewEdgeRoute(edge, movedNodes, exact)!;

    expect(preview.points[0]).toEqual({ x: 120, y: 110 });
    expect(preview.points.at(-1)).toEqual({ x: 280, y: 30 });
    expect(preview.points.slice(1).every((point, index) => {
      const previous = preview.points[index];
      return previous.x === point.x || previous.y === point.y;
    })).toBe(true);
  });
});
