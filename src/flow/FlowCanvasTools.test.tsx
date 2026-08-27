import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { FlowCanvasTools } from './FlowCanvasTools';
import { createFlowStore, FlowProvider } from './store';

describe('FlowCanvasTools', () => {
  it('requests pan mode and reflects the controlled active tool', () => {
    const onModeChange = vi.fn();
    const store = createFlowStore();
    const view = render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          canvasSize={{ width: 800, height: 600 }}
          mode="select"
          onModeChange={onModeChange}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '平移' }));
    expect(onModeChange).toHaveBeenCalledWith('pan');

    view.rerender(
      <FlowProvider store={store}>
        <FlowCanvasTools
          canvasSize={{ width: 800, height: 600 }}
          mode="pan"
          onModeChange={onModeChange}
        />
      </FlowProvider>,
    );
    expect(screen.getByRole('button', { name: '平移' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('shows canvas shortcut guidance from the settings control', () => {
    const store = createFlowStore();
    render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          canvasSize={{ width: 800, height: 600 }}
          mode="select"
          onModeChange={vi.fn()}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '画布设置' }));

    expect(screen.getByRole('heading', { name: '画布快捷操作' })).toBeVisible();
    expect(screen.getByText('移动选中节点 1 像素')).toBeVisible();
  });

  it('locates the selected node without changing zoom', () => {
    const store = createFlowStore({
      nodes: [{
        id: 'selected',
        kind: 'test',
        position: { x: 100, y: 200 },
        size: { width: 120, height: 50 },
        data: null,
      }],
      edges: [],
      metadata: {},
    });
    store.setState({
      selectedNodeIds: new Set(['selected']),
      viewport: { x: 4, y: 8, zoom: 1.5 },
    });
    render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          canvasSize={{ width: 800, height: 600 }}
          mode="select"
          onModeChange={vi.fn()}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '居中显示' }));

    expect(store.getState().viewport).toEqual({ x: 160, y: -37.5, zoom: 1.5 });
  });

  it('fits all nodes and changes zoom', () => {
    const store = createFlowStore({
      nodes: [{
        id: 'large',
        kind: 'test',
        position: { x: -300, y: -100 },
        size: { width: 1_000, height: 600 },
        data: null,
      }],
      edges: [],
      metadata: {},
    });
    render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          canvasSize={{ width: 800, height: 600 }}
          mode="select"
          onModeChange={vi.fn()}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '显示全部' }));

    expect(store.getState().viewport.zoom).toBeCloseTo(0.656);
    expect(store.getState().viewport).not.toEqual({ x: 0, y: 42, zoom: 1 });
  });
});
