import { describe, expect, it } from 'vitest';

import { routeEdge } from './routing';
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
});
