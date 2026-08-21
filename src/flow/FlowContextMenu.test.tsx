import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, FlowProvider } from './store';
import type { FlowNode, NodeRegistry } from './types';
import { FlowContextMenu } from './FlowContextMenu';

/** 测试节点无需视觉内容。 */
function EmptyNode() {
  return null;
}

/** 测试菜单使用的最小节点注册表。 */
const registry = {
  start: {
    kind: 'start',
    title: '开始',
    defaultSize: { width: 144, height: 50 },
    singleton: true,
    component: EmptyNode,
  },
  log: {
    kind: 'log',
    title: '日志',
    defaultSize: { width: 168, height: 52 },
    component: EmptyNode,
  },
} satisfies NodeRegistry;

/** 构造具有稳定位置的菜单测试节点。 */
const createNode = (id: string, kind: string, x: number): FlowNode => ({
  id,
  kind,
  position: { x, y: 0 },
  size: { width: 100, height: 50 },
  data: null,
});

describe('FlowContextMenu', () => {
  it('opens the node submenu, disables singleton nodes and adds an available node', () => {
    const store = createFlowStore({ nodes: [createNode('start', 'start', 0)] });
    const onAddNode = vi.fn();
    const onClose = vi.fn();

    render(
      <FlowProvider store={store}>
        <FlowContextMenu
          context={{ x: 10, y: 12, world: { x: 40, y: 60 }, submenuSide: 'right' }}
          registry={registry}
          nodes={store.getState().nodes}
          onAddNode={onAddNode}
          onClose={onClose}
        />
      </FlowProvider>,
    );

    expect(screen.getByRole('menuitem', { name: /复制/ })).toBeDisabled();
    fireEvent.click(screen.getByRole('menuitem', { name: '添加节点' }));
    expect(screen.getByRole('menuitem', { name: '开始' })).toBeDisabled();
    fireEvent.click(screen.getByRole('menuitem', { name: '日志' }));

    expect(onAddNode).toHaveBeenCalledWith('log', { x: 40, y: 60 });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('runs a vertical align command for multiple selected nodes', () => {
    const nodes = [createNode('a', 'log', 0), createNode('b', 'log', 100)];
    const store = createFlowStore({ nodes });
    store.getState().selectNodes(nodes.map((node) => node.id));

    render(
      <FlowProvider store={store}>
        <FlowContextMenu
          context={{ x: 10, y: 12, world: { x: 40, y: 60 }, submenuSide: 'right' }}
          registry={registry}
          nodes={nodes}
          onAddNode={vi.fn()}
          onClose={vi.fn()}
        />
      </FlowProvider>,
    );

    fireEvent.keyDown(screen.getByRole('menuitem', { name: '排列与对齐' }), {
      key: 'ArrowRight',
    });
    fireEvent.click(screen.getByRole('menuitem', { name: '左对齐' }));

    expect(store.getState().nodes.map((node) => node.position.x)).toEqual([0, 0]);
  });

  it('cycles focus with desktop keys and closes with Escape', () => {
    const store = createFlowStore();
    const onClose = vi.fn();

    render(
      <FlowProvider store={store}>
        <FlowContextMenu
          context={{ x: 10, y: 12, world: { x: 40, y: 60 }, submenuSide: 'right' }}
          registry={registry}
          nodes={[]}
          onAddNode={vi.fn()}
          onClose={onClose}
        />
      </FlowProvider>,
    );

    const menu = screen.getByRole('menu', { name: '画布菜单' });
    fireEvent.keyDown(menu, { key: 'End' });
    expect(document.activeElement).toHaveAccessibleName('添加节点');
    fireEvent.keyDown(menu, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
