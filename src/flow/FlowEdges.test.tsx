import { act, render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, FlowProvider } from './store';
import type { FlowEdge, FlowNode, RoutedEdge } from './types';
import { FlowEdges } from './FlowEdges';

/** 测试固定路由，避免运行态 SVG 断言依赖异步 Worker。 */
const route: RoutedEdge = {
  edgeId: 'edge-1',
  points: [{ x: 80, y: 25 }, { x: 200, y: 25 }],
  path: 'M 80 25 L 200 25',
  sourceSide: 'right',
  targetSide: 'left',
  bounds: { x: 80, y: 25, width: 120, height: 1 },
};

vi.mock('./useEdgeRoutes', () => ({
  useEdgeRoutes: () => [route],
}));

const nodes: FlowNode[] = [
  {
    id: 'source',
    kind: 'test',
    position: { x: 0, y: 0 },
    size: { width: 80, height: 50 },
    data: null,
  },
  {
    id: 'target',
    kind: 'test',
    position: { x: 200, y: 0 },
    size: { width: 80, height: 50 },
    data: null,
  },
];

const edges: FlowEdge[] = [{
  id: 'edge-1',
  source: { nodeId: 'source' },
  target: { nodeId: 'target' },
  data: null,
}];

describe('FlowEdges runtime pulse', () => {
  it('renders a directional pulse only for an active edge', () => {
    const store = createFlowStore({ nodes, edges, metadata: {} });
    store.setState({ activeEdgeIds: { 'edge-1': Date.now() + 900 } });
    const view = render(
      <FlowProvider store={store}>
        <FlowEdges
          height={600}
          width={800}
          panActive={false}
          onReconnectStart={vi.fn()}
        />
      </FlowProvider>,
    );

    const pulse = view.container.querySelector('[data-flow-edge-runtime="edge-1"]');
    expect(pulse).not.toBeNull();
    expect(pulse).toHaveAttribute('stroke-dasharray', '2 11');
    expect(pulse).toHaveClass('motion-reduce:hidden');

    act(() => store.setState({ activeEdgeIds: {} }));
    expect(view.container.querySelector('[data-flow-edge-runtime]')).toBeNull();
  });
});
