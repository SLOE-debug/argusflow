import { describe, expect, it } from 'vitest';

import { ObstacleIndex } from './obstacleIndex';
import { compactRouteCore } from './routeCompaction';
import { isRouteCoreClear } from './routeCollision';
import type { FlowNode, FlowPoint } from './types';

/** 创建阻断两条直接 L 形候选的中央障碍物。 */
function createBlockingNode(): FlowNode {
  return {
    id: 'blocker',
    kind: 'test',
    position: { x: 80, y: 40 },
    size: { width: 40, height: 100 },
    data: null,
  };
}

describe('compactRouteCore', () => {
  it('replaces a collision-free micro staircase with one safe channel', () => {
    const obstacles = new ObstacleIndex();
    obstacles.syncAll([createBlockingNode()]);
    const collision = {
      obstacles,
      endpointNodeIds: new Set<string>(),
      endpointKeepOutRects: [],
    };
    /** 模拟 OVG 在障碍物上方生成的连续短距离折点。 */
    const staircase: ReadonlyArray<FlowPoint> = [
      { x: 0, y: 50 },
      { x: 40, y: 50 },
      { x: 40, y: 20 },
      { x: 60, y: 20 },
      { x: 60, y: 0 },
      { x: 140, y: 0 },
      { x: 140, y: 20 },
      { x: 160, y: 20 },
      { x: 160, y: 150 },
      { x: 200, y: 150 },
    ];

    const compacted = compactRouteCore(staircase, collision);

    expect(compacted).toEqual([
      { x: 0, y: 50 },
      { x: 0, y: 20 },
      { x: 200, y: 20 },
      { x: 200, y: 150 },
    ]);
    expect(isRouteCoreClear(compacted, collision)).toBe(true);
  });

  it('removes a local staircase when the complete route needs more bends', () => {
    const obstacles = new ObstacleIndex();
    obstacles.syncAll([
      {
        id: 'upper-wall',
        kind: 'test',
        position: { x: 80, y: -100 },
        size: { width: 40, height: 120 },
        data: null,
      },
      {
        id: 'lower-wall',
        kind: 'test',
        position: { x: 240, y: -20 },
        size: { width: 40, height: 120 },
        data: null,
      },
    ]);
    const collision = {
      obstacles,
      endpointNodeIds: new Set<string>(),
      endpointKeepOutRects: [],
    };
    const staircase: ReadonlyArray<FlowPoint> = [
      { x: 0, y: 0 },
      { x: 0, y: 50 },
      { x: 150, y: 50 },
      { x: 150, y: 45 },
      { x: 160, y: 45 },
      { x: 160, y: 40 },
      { x: 180, y: 40 },
      { x: 180, y: -50 },
      { x: 320, y: -50 },
      { x: 320, y: 0 },
      { x: 400, y: 0 },
    ];

    const compacted = compactRouteCore(staircase, collision);

    expect(compacted.length).toBeLessThan(staircase.length);
    expect(compacted).not.toContainEqual({ x: 150, y: 45 });
    expect(compacted).not.toContainEqual({ x: 160, y: 40 });
    expect(isRouteCoreClear(compacted, collision)).toBe(true);
  });
});
