import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { FlowCanvas } from './FlowCanvas';
import { createFlowStore, FlowProvider } from '../store/store';
import type { FlowNode, FlowNodeRendererProps, NodeRegistry } from '../types';

/** 测试节点只显示 ID，交互由通用节点外壳负责。 */
function TestNode({ node }: FlowNodeRendererProps) {
  return <span>{node.id}</span>;
}

const registry = {
  test: {
    kind: 'test',
    title: '测试',
    defaultSize: { width: 80, height: 50 },
    component: TestNode,
    resizable: true,
    minSize: { width: 60, height: 40 },
  },
} satisfies NodeRegistry;

/** 创建用于手势和连线命中的固定尺寸节点。 */
const createNode = (id: string, x: number): FlowNode => ({
  id,
  kind: 'test',
  position: { x, y: 20 },
  size: { width: 80, height: 50 },
  data: null,
});

describe('FlowCanvas interactions', () => {
  beforeEach(() => {
    let frameId = 0;
    vi.stubGlobal('requestAnimationFrame', vi.fn(() => ++frameId));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    vi.stubGlobal('ResizeObserver', class {
      public constructor(private readonly callback: ResizeObserverCallback) {}

      public observe(): void {
        this.callback([{
          contentRect: { width: 800, height: 600 },
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

  afterEach(() => {
    Reflect.deleteProperty(document, 'elementFromPoint');
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('prevents browser text selection when box selection starts', () => {
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    const removeAllRanges = vi.fn();
    vi.spyOn(window, 'getSelection').mockReturnValue({
      removeAllRanges,
    } as unknown as Selection);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const canvas = container.querySelector('.touch-none');
    expect(canvas).toHaveClass('select-none');

    const eventWasNotCancelled = fireEvent.pointerDown(canvas!, {
      button: 0,
      clientX: 300,
      clientY: 200,
    });

    expect(eventWasNotCancelled).toBe(false);
    expect(removeAllRanges).toHaveBeenCalledOnce();
  });

  it('accepts a palette node when WebView hides drag types until drop', () => {
    const store = createFlowStore({ nodes: [], edges: [] });
    const onAddNode = vi.fn();
    /** 模拟 WebView 在 dragover 阶段隐藏类型、在 drop 阶段才允许读取的负载。 */
    const dragPayload = new Map([
      ['text/plain', 'argusflow-node:test'],
    ]);
    const dataTransfer = {
      types: [],
      dropEffect: 'none',
      getData: (type: string) => dragPayload.get(type) ?? '',
    } as unknown as DataTransfer;
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={onAddNode}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const canvas = container.querySelector('.touch-none');

    expect(fireEvent.dragOver(canvas!, { dataTransfer })).toBe(false);
    expect(dataTransfer.dropEffect).toBe('copy');
    fireEvent.drop(canvas!, {
      clientX: 120,
      clientY: 110,
      dataTransfer,
    });

    expect(onAddNode).toHaveBeenCalledWith('test', expect.any(Object));
  });

  it('pans from a selected node while Space is pressed', () => {
    const nodes = [createNode('a', 20), createNode('b', 160)];
    const store = createFlowStore({ nodes, edges: [] });
    store.getState().selectNodes(['a', 'b']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    fireEvent.pointerEnter(container.querySelector('.touch-none')!);
    fireEvent.keyDown(window, { code: 'Space', key: ' ' });
    const selectedNode = container.querySelector('[data-flow-node-id="a"]');
    expect(selectedNode).not.toBeNull();

    fireEvent.pointerDown(selectedNode!, {
      button: 0,
      clientX: 40,
      clientY: 80,
    });
    fireEvent.pointerMove(window, { clientX: 70, clientY: 100 });
    fireEvent.pointerMove(window, { clientX: 90, clientY: 120 });
    fireEvent.pointerUp(window, { clientX: 90, clientY: 120 });

    expect(store.getState().viewport).toEqual({ x: 50, y: 82, zoom: 1 });
    expect(store.getState().nodes).toBe(nodes);
    expect(store.getState().selectedNodeIds).toEqual(new Set(['a', 'b']));
    expect(store.getState().past).toHaveLength(0);
    fireEvent.keyUp(window, { code: 'Space', key: ' ' });
  });

  it('nudges selected nodes with arrow keys and groups repeated moves', () => {
    const nodes = [createNode('a', 20), createNode('b', 160)];
    const store = createFlowStore({ nodes, edges: [] });
    store.getState().selectNodes(['a', 'b']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );

    fireEvent.pointerEnter(container.querySelector('.touch-none')!);
    const rightWasNotCancelled = fireEvent.keyDown(window, {
      key: 'ArrowRight',
    });
    fireEvent.keyDown(window, { key: 'ArrowUp', shiftKey: true });

    expect(rightWasNotCancelled).toBe(false);
    expect(store.getState().nodes.map((item) => item.position)).toEqual([
      { x: 21, y: 10 },
      { x: 161, y: 10 },
    ]);
    expect(store.getState().past).toHaveLength(1);

    store.getState().undo();
    expect(store.getState().nodes).toBe(nodes);
  });

  it('resizes a selected node as one undoable gesture and respects minimum size', () => {
    const nodes = [createNode('a', 20)];
    const store = createFlowStore({ nodes, edges: [] });
    store.getState().selectNodes(['a']);
    render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );

    fireEvent.pointerDown(screen.getByRole('button', { name: '调整 a 大小' }), {
      button: 0,
      clientX: 100,
      clientY: 70,
    });
    fireEvent.pointerUp(window, { clientX: 40, clientY: 20 });

    expect(store.getState().nodes[0]?.size).toEqual({ width: 60, height: 40 });
    expect(store.getState().past).toHaveLength(1);
    store.getState().undo();
    expect(store.getState().nodes).toBe(nodes);
  });

  it('activates the single selected structure with Enter', () => {
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    const onNodeDoubleClick = vi.fn();
    store.getState().selectNodes(['a']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
          onNodeDoubleClick={onNodeDoubleClick}
        />
      </FlowProvider>,
    );

    fireEvent.pointerEnter(container.querySelector('.touch-none')!);
    expect(fireEvent.keyDown(window, { key: 'Enter' })).toBe(false);
    expect(onNodeDoubleClick).toHaveBeenCalledWith('a');
  });

  it('delegates zoom threshold crossings to semantic scope navigation', () => {
    let wheelFrame: FrameRequestCallback | undefined;
    vi.mocked(requestAnimationFrame).mockImplementation((callback) => {
      wheelFrame = callback;
      return 1;
    });
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    const onSemanticZoomIn = vi.fn(() => true);
    const onSemanticZoomOut = vi.fn(() => true);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
          onSemanticZoomIn={onSemanticZoomIn}
          onSemanticZoomOut={onSemanticZoomOut}
        />
      </FlowProvider>,
    );
    const canvas = container.querySelector('.touch-none');

    expect(fireEvent.wheel(canvas!, {
      clientX: 40,
      clientY: 40,
      deltaY: -1_000,
    })).toBe(false);
    wheelFrame?.(0);
    expect(onSemanticZoomIn).toHaveBeenCalled();
    expect(onSemanticZoomOut).not.toHaveBeenCalled();

    onSemanticZoomIn.mockReturnValue(false);
    fireEvent.wheel(canvas!, { clientX: 40, clientY: 40, deltaY: 2_000 });
    wheelFrame?.(1);
    expect(onSemanticZoomOut).toHaveBeenCalled();
  });

  it('preserves native arrow-key behavior in editable and menu controls', () => {
    const nodes = [createNode('a', 20)];
    const store = createFlowStore({ nodes, edges: [] });
    store.getState().selectNodes(['a']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const input = document.createElement('input');
    container.append(input);
    fireEvent.pointerEnter(container.querySelector('.touch-none')!);

    expect(fireEvent.keyDown(input, { key: 'ArrowRight' })).toBe(true);
    fireEvent.contextMenu(container.querySelector('.touch-none')!, {
      clientX: 100,
      clientY: 100,
    });
    const menu = screen.getByRole('menu', { name: '画布菜单' });
    expect(fireEvent.keyDown(menu, { key: 'ArrowDown' })).toBe(false);

    expect(store.getState().nodes).toBe(nodes);
    expect(store.getState().past).toHaveLength(0);
  });

  it('preserves native copy for selected text without disabling canvas copy', () => {
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    store.getState().selectNodes(['a']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    /** 模拟用户在底部运行日志中框选了一段可复制文本。 */
    const selectableText = document.createElement('p');
    selectableText.textContent = '可复制的运行日志';
    container.append(selectableText);
    const selection = window.getSelection();
    const textRange = document.createRange();
    textRange.selectNodeContents(selectableText);
    selection?.removeAllRanges();
    selection?.addRange(textRange);
    fireEvent.pointerEnter(container.querySelector('.touch-none')!);

    expect(selection?.isCollapsed).toBe(false);
    expect(fireEvent.keyDown(selectableText, { key: 'c', ctrlKey: true })).toBe(true);
    expect(store.getState().clipboard).toBeNull();

    selection?.removeAllRanges();
    expect(fireEvent.keyDown(window, { key: 'c', ctrlKey: true })).toBe(false);
    expect(store.getState().clipboard?.nodes.map((node) => node.id)).toEqual(['a']);
  });

  it('only captures clipboard and Space shortcuts while the pointer is inside the canvas', () => {
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    store.getState().selectNodes(['a']);
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const canvas = container.querySelector('.touch-none');

    expect(fireEvent.keyDown(window, { key: 'c', ctrlKey: true })).toBe(true);
    expect(store.getState().clipboard).toBeNull();

    fireEvent.pointerEnter(canvas!);
    expect(fireEvent.keyDown(window, { key: 'c', ctrlKey: true })).toBe(false);
    expect(store.getState().clipboard?.nodes.map((node) => node.id)).toEqual(['a']);
    expect(fireEvent.keyDown(window, { code: 'Space', key: ' ' })).toBe(false);
    expect(canvas).toHaveClass('cursor-grab');

    fireEvent.pointerLeave(canvas!);
    expect(canvas).toHaveClass('cursor-crosshair');
    /** 画布外的原生粘贴不能创建 Flow 节点副本。 */
    const nodesBeforeNativePaste = store.getState().nodes;
    expect(fireEvent.keyDown(window, { key: 'v', ctrlKey: true })).toBe(true);
    expect(fireEvent.keyDown(window, { code: 'Space', key: ' ' })).toBe(true);
    expect(store.getState().nodes).toBe(nodesBeforeNativePaste);
  });

  it('lets the transparent edge path receive hover and selection events', () => {
    const nodes = [createNode('a', 0), createNode('b', 120)];
    const store = createFlowStore({
      nodes,
      edges: [{
        id: 'edge',
        source: { nodeId: 'a', side: 'right' },
        target: { nodeId: 'b', side: 'left' },
        data: null,
      }],
    });
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const hitPath = container.querySelector('[data-flow-edge-hit="edge"]');
    const visiblePath = container.querySelector('#path-edge');
    const nodeWorld = container.querySelector('.z-10.origin-top-left');
    expect(hitPath).toHaveClass('[pointer-events:stroke]');
    expect(nodeWorld).toHaveClass('pointer-events-none');

    fireEvent.pointerEnter(hitPath!);
    expect(visiblePath).toHaveAttribute('stroke', '#7c3aed');
    fireEvent.pointerDown(hitPath!, { button: 0 });
    expect(store.getState().selectedEdgeId).toBe('edge');
    fireEvent.pointerLeave(hitPath!);
    expect(visiblePath).toHaveAttribute('stroke', '#0f766e');
  });

  it('opens node creation when a connection ends on empty canvas', () => {
    const store = createFlowStore({ nodes: [createNode('a', 20)], edges: [] });
    const onAddConnectedNode = vi.fn(() => true);
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => null),
    });
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={onAddConnectedNode}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    const node = container.querySelector('[data-flow-node-id="a"]');
    fireEvent.pointerEnter(node!);
    fireEvent.pointerDown(screen.getByRole('button', { name: 'a right 锚点' }), {
      button: 0,
      clientX: 100,
      clientY: 80,
    });
    fireEvent.pointerUp(window, { clientX: 300, clientY: 200 });

    expect(screen.getByRole('menu', { name: '添加并连接节点' })).toBeVisible();
    fireEvent.click(screen.getByRole('menuitem', { name: '测试' }));
    expect(onAddConnectedNode).toHaveBeenCalledWith(
      'test',
      { x: 260, y: 133 },
      'a',
      'right',
    );
  });

  it('pans from an edge without selecting it when the pan tool is active', () => {
    const nodes = [createNode('a', 0), createNode('b', 120)];
    const store = createFlowStore({
      nodes,
      edges: [{
        id: 'edge',
        source: { nodeId: 'a', side: 'right' },
        target: { nodeId: 'b', side: 'left' },
        data: null,
      }],
    });
    const { container } = render(
      <FlowProvider store={store}>
        <FlowCanvas
          registry={registry}
          onAddNode={vi.fn()}
          onAddConnectedNode={() => true}
          onConnect={() => true}
          onReconnect={() => true}
        />
      </FlowProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: '平移' }));
    const hitPath = container.querySelector('[data-flow-edge-hit="edge"]');

    fireEvent.pointerDown(hitPath!, {
      button: 0,
      clientX: 100,
      clientY: 80,
    });
    fireEvent.pointerMove(window, { clientX: 130, clientY: 100 });
    fireEvent.pointerUp(window, { clientX: 130, clientY: 100 });

    expect(store.getState().viewport).toEqual({ x: 30, y: 62, zoom: 1 });
    expect(store.getState().selectedEdgeId).toBeNull();
  });
});
