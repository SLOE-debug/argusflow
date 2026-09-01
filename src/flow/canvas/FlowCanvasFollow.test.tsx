import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { FlowCanvas } from './FlowCanvas';
import { createFlowStore, FlowProvider } from '../store/store';
import type { FlowNodeRendererProps, NodeRegistry } from '../types';

function TestNode({ node }: FlowNodeRendererProps) {
  return <span>{node.id}</span>;
}

const registry = {
  test: {
    kind: 'test',
    title: '测试节点',
    defaultSize: { width: 80, height: 50 },
    component: TestNode,
  },
} satisfies NodeRegistry;

describe('FlowCanvas follow bounds', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', class {
      public constructor(private readonly callback: ResizeObserverCallback) {}

      public observe(): void {
        this.callback([{
          contentRect: { width: 400, height: 300 },
        } as ResizeObserverEntry], this as unknown as ResizeObserver);
      }

      public disconnect(): void {}
      public unobserve(): void {}
    });
    vi.stubGlobal('Worker', class {
      public postMessage(): void {}
      public addEventListener(): void {}
      public removeEventListener(): void {}
      public terminate(): void {}
    });
  });

  it('pans in both axes when the execution target is outside the safe viewport', () => {
    const store = createFlowStore({ nodes: [], edges: [] });
    render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          interactionMode="readonly"
          followBounds={{ x: 500, y: 420, width: 80, height: 50 }}
          followPadding={{ top: 40, right: 50, bottom: 60, left: 30 }}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => false}
          onConnect={() => false}
          onReconnect={() => false}
        />
      </FlowProvider>,
    );

    expect(store.getState().viewport).toEqual({ x: -230, y: -230, zoom: 1 });
  });
});
