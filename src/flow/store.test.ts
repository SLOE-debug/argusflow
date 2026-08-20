import { describe, expect, it, vi } from 'vitest';

import { createFlowStore } from './store';
import type { FlowNode } from './types';

const node = (id: string, kind = 'log'): FlowNode => ({ id, kind, position: { x: 0, y: 0 }, size: { width: 40, height: 30 }, data: null });

describe('flow store history and clipboard', () => {
  it('undoes and redoes document transactions', () => {
    const store = createFlowStore({ nodes: [node('a')], edges: [] });
    store.getState().transact((document) => ({ ...document, nodes: [...document.nodes, node('b')] }));
    expect(store.getState().nodes).toHaveLength(2);
    store.getState().undo();
    expect(store.getState().nodes).toHaveLength(1);
    store.getState().redo();
    expect(store.getState().nodes).toHaveLength(2);
  });

  it('copies internal edges and skips singleton conflicts', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'copy' });
    const store = createFlowStore({
      nodes: [node('start', 'start'), node('a')],
      edges: [{ id: 'edge', source: { nodeId: 'start' }, target: { nodeId: 'a' }, data: null }],
    });
    store.getState().selectNodes(['start', 'a']);
    store.getState().copy();
    store.getState().paste(new Set(['start']));
    expect(store.getState().nodes.filter((item) => item.kind === 'start')).toHaveLength(1);
    expect(store.getState().nodes).toHaveLength(3);
    expect(store.getState().edges).toHaveLength(1);
    vi.unstubAllGlobals();
  });
});
