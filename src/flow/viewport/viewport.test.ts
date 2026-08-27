import { describe, expect, it } from 'vitest';

import type { FlowNode } from '../types';
import {
  MAX_CANVAS_ZOOM,
  centerBoundsInViewport,
  fitBoundsToViewport,
  getNodesBounds,
} from './viewport';

/** 创建仅用于视口数学测试的最小 Flow 节点。 */
function createTestNode(
  id: string,
  x: number,
  y: number,
  width: number,
  height: number,
): FlowNode {
  return {
    id,
    kind: 'test',
    position: { x, y },
    size: { width, height },
    data: null,
  };
}

describe('flow viewport', () => {
  it('returns no bounds for an empty canvas', () => {
    expect(getNodesBounds([])).toBeNull();
  });

  it('calculates bounds for one node', () => {
    expect(getNodesBounds([
      createTestNode('node', 100, 200, 120, 50),
    ])).toEqual({ x: 100, y: 200, width: 120, height: 50 });
  });

  it('includes negative and distant node coordinates', () => {
    expect(getNodesBounds([
      createTestNode('left', -300, -100, 100, 50),
      createTestNode('right', 500, 400, 200, 100),
    ])).toEqual({ x: -300, y: -100, width: 1_000, height: 600 });
  });

  it('centers bounds while retaining the requested zoom', () => {
    expect(centerBoundsInViewport(
      { x: 100, y: 200, width: 120, height: 50 },
      { width: 800, height: 600 },
      1.5,
    )).toEqual({ x: 160, y: -37.5, zoom: 1.5 });
  });

  it('caps tiny content at the canvas maximum zoom', () => {
    const viewport = fitBoundsToViewport(
      { x: 0, y: 0, width: 1, height: 1 },
      { width: 800, height: 600 },
      { padding: 72, maxZoom: MAX_CANVAS_ZOOM },
    );

    expect(viewport.zoom).toBe(MAX_CANVAS_ZOOM);
  });

  it('fits large content inside the requested padding', () => {
    const bounds = { x: -300, y: -100, width: 1_000, height: 600 };
    const viewport = fitBoundsToViewport(
      bounds,
      { width: 800, height: 600 },
      { padding: 72 },
    );
    /** 转换后内容四边，用于验证都落在视口留白范围内。 */
    const screenBounds = {
      left: bounds.x * viewport.zoom + viewport.x,
      top: bounds.y * viewport.zoom + viewport.y,
      right: (bounds.x + bounds.width) * viewport.zoom + viewport.x,
      bottom: (bounds.y + bounds.height) * viewport.zoom + viewport.y,
    };

    expect(screenBounds.left).toBeGreaterThanOrEqual(72 - Number.EPSILON * 512);
    expect(screenBounds.top).toBeGreaterThanOrEqual(72 - Number.EPSILON * 512);
    expect(screenBounds.right).toBeLessThanOrEqual(800 - 72 + Number.EPSILON * 512);
    expect(screenBounds.bottom).toBeLessThanOrEqual(600 - 72 + Number.EPSILON * 512);
  });
});
