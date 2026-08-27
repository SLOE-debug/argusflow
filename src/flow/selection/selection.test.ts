import { describe, expect, it } from 'vitest';

import { alignNodes, distributeNodes } from './selection';
import type { FlowNode } from '../types';

const nodes: FlowNode[] = [0, 1, 2].map((index) => ({ id: String(index), kind: 'test', position: { x: index * 90, y: index * 40 }, size: { width: 20, height: 20 }, data: null }));

describe('flow selection transforms', () => {
  it('aligns selected nodes without moving unselected nodes', () => {
    const next = alignNodes(nodes, new Set(['0', '1']), 'left');
    expect(next[1].position.x).toBe(0);
    expect(next[2]).toBe(nodes[2]);
  });

  it('distributes centers evenly', () => {
    const uneven = [{ ...nodes[0] }, { ...nodes[1], position: { x: 30, y: 40 } }, { ...nodes[2] }];
    const next = distributeNodes(uneven, new Set(['0', '1', '2']), 'horizontal');
    expect(next[1].position.x).toBe(90);
  });
});
