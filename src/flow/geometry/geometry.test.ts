import { describe, expect, it } from 'vitest';

import { screenToWorld, worldToScreen, zoomAt } from './geometry';

describe('flow coordinates', () => {
  it('round trips coordinates and keeps zoom origin stable', () => {
    const viewport = { x: 40, y: 20, zoom: 1.5 };
    const screen = worldToScreen({ x: 100, y: 60 }, viewport);
    expect(screenToWorld(screen, viewport)).toEqual({ x: 100, y: 60 });
    const next = zoomAt(viewport, screen, 2);
    expect(screenToWorld(screen, next)).toEqual({ x: 100, y: 60 });
  });
});
