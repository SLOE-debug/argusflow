import { describe, expect, it } from 'vitest';

import { snapNode } from './snapping';
import type { FlowNode } from './types';

/** 构造吸附测试所需的最小节点。 */
function node(id: string, x: number, y: number): FlowNode {
  return {
    id,
    kind: 'test',
    position: { x, y },
    size: { width: 100, height: 50 },
    data: null,
  };
}

describe('snapNode', () => {
  it('connects aligned nodes with a guide between their outer edges', () => {
    const result = snapNode(
      node('moving', 104, 200),
      [node('reference', 100, 20)],
      6,
    );

    expect(result.position.x).toBe(100);
    expect(result.guides).toEqual([{
      axis: 'x',
      value: 100,
      start: 70,
      end: 200,
      kind: 'start',
    }]);
  });

  it('shows a spacing guide only at the recommended node gap', () => {
    const snapped = snapNode(
      node('moving', 223, 100),
      [node('reference', 100, 100)],
      4,
    );
    const movedCloser = snapNode(
      node('moving', 218, 100),
      [node('reference', 100, 100)],
      4,
    );

    expect(snapped.position).toEqual({ x: 224, y: 100 });
    expect(snapped.guides).toContainEqual({
      axis: 'y',
      value: 125,
      start: 200,
      end: 224,
      kind: 'spacing',
    });
    expect(movedCloser.position.x).toBe(218);
    expect(movedCloser.guides).toEqual([]);
  });

  it('identifies alignment independently on each axis', () => {
    const result = snapNode(
      node('moving', 103, 102),
      [
        node('vertical-reference', 100, 300),
        node('horizontal-reference', 400, 100),
      ],
      4,
    );

    expect(result.position).toEqual({ x: 100, y: 100 });
    expect(result.guides).toHaveLength(2);
  });
});
