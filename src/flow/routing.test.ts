import { describe, expect, it } from 'vitest';

import { previewEdgeRoute, routeEdge } from './routing';
import { rectsIntersect } from './geometry';
import type { FlowEdge, FlowNode, FlowPoint } from './types';

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

  it('keeps a safe straight segment before turning at both endpoints', () => {
    const nodes: FlowNode[] = [
      { id: 'a', kind: 'test', position: { x: 0, y: 0 }, size: { width: 80, height: 60 }, data: null },
      { id: 'b', kind: 'test', position: { x: 0, y: 200 }, size: { width: 80, height: 60 }, data: null },
    ];
    const edge: FlowEdge = {
      id: 'edge',
      source: { nodeId: 'a', side: 'right' },
      target: { nodeId: 'b', side: 'right' },
      data: null,
    };

    const exact = routeEdge(edge, nodes)!;
    const preview = previewEdgeRoute(edge, nodes);

    expect(exact.points.slice(0, 2)).toEqual([
      { x: 80, y: 30 },
      { x: 94, y: 30 },
    ]);
    expect(exact.points.slice(-2)).toEqual([
      { x: 94, y: 230 },
      { x: 80, y: 230 },
    ]);
    expect(preview?.points).toEqual(exact.points);
  });

  it('routes a backward connection around intervening nodes', () => {
    const blocker: FlowNode = {
      id: 'blocker',
      kind: 'test',
      position: { x: 200, y: 0 },
      size: { width: 120, height: 60 },
      data: null,
    };
    const nodes: FlowNode[] = [
      { id: 'source', kind: 'test', position: { x: 400, y: 0 }, size: { width: 120, height: 60 }, data: null },
      blocker,
      { id: 'target', kind: 'test', position: { x: 200, y: 180 }, size: { width: 120, height: 60 }, data: null },
    ];
    const edge: FlowEdge = {
      id: 'edge',
      source: { nodeId: 'source', side: 'right' },
      target: { nodeId: 'target', side: 'left' },
      data: null,
    };

    const exact = routeEdge(edge, nodes)!;
    const preview = previewEdgeRoute(edge, nodes)!;
    const blockerRect = { ...blocker.position, ...blocker.size };
    const crossesBlocker = (points: ReadonlyArray<FlowPoint>) => points.slice(1).some((point, index) => {
      const previous = points[index];
      return rectsIntersect({
        x: Math.min(previous.x, point.x),
        y: Math.min(previous.y, point.y),
        width: Math.max(1, Math.abs(previous.x - point.x)),
        height: Math.max(1, Math.abs(previous.y - point.y)),
      }, blockerRect);
    });

    expect(preview.points[1]).toEqual({ x: 534, y: 30 });
    expect(preview.points.at(-2)).toEqual({ x: 186, y: 210 });
    expect(crossesBlocker(exact.points)).toBe(false);
    expect(crossesBlocker(preview.points)).toBe(false);
  });
});
