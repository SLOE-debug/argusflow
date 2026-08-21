import {
  useCallback,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type WheelEvent as ReactWheelEvent,
} from 'react';

import { rectFromPoints, rectsIntersect, screenToWorld, zoomAt } from './geometry';
import type { CanvasToolMode } from './FlowCanvasTools';
import { snapNode, type AlignmentGuide } from './snapping';
import { useFlowStoreApi } from './store';
import type { FlowAnchorSide, FlowPoint } from './types';

type ConnectionEndpoint = 'source' | 'target';

type ReconnectTarget = Readonly<{
  edgeId: string;
  endpoint: ConnectionEndpoint;
}>;

type UseCanvasPointerInteractionsOptions = Readonly<{
  containerRef: RefObject<HTMLDivElement | null>;
  maxZoom: number;
  onConnect: (
    source: string,
    target: string,
    sourceSide?: FlowAnchorSide,
    targetSide?: FlowAnchorSide,
  ) => boolean;
  onReconnect: (
    edgeId: string,
    endpoint: ConnectionEndpoint,
    nodeId: string,
    side?: FlowAnchorSide,
  ) => boolean;
  spacePressed: boolean;
  toolMode: CanvasToolMode;
}>;

/** 画布右键菜单的屏幕位置、世界坐标和二级菜单方向。 */
export type CanvasContextMenu = Readonly<{
  x: number;
  y: number;
  world: FlowPoint;
  submenuSide: 'left' | 'right';
}>;

/** 画布对外暴露的具名指针手势和临时视觉状态。 */
type CanvasPointerInteractions = Readonly<{
  closeContextMenu: () => void;
  contextMenu: CanvasContextMenu | null;
  guides: ReadonlyArray<AlignmentGuide>;
  handleConnectionStart: (
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  handleContextMenu: (event: ReactMouseEvent<HTMLDivElement>) => void;
  handleNodeDragStart: (nodeId: string, event: ReactPointerEvent) => void;
  handlePanePointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
  handleReconnectStart: (
    edgeId: string,
    endpoint: ConnectionEndpoint,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  handleWheel: (event: ReactWheelEvent<HTMLDivElement>) => void;
}>;

const FLOW_ANCHOR_SIDES: ReadonlySet<string> = new Set([
  'top',
  'right',
  'bottom',
  'left',
]);

/** 将 DOM 数据属性收窄为 Flow 锚点枚举。 */
function isFlowAnchorSide(value: string | undefined): value is FlowAnchorSide {
  return value !== undefined && FLOW_ANCHOR_SIDES.has(value);
}

/** 绑定一组仅在当前拖拽手势期间存在的全局指针监听。 */
function bindPointerGesture(
  move: (event: PointerEvent) => void,
  finish: (event: PointerEvent) => void,
): void {
  const handlePointerUp = (event: PointerEvent) => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', handlePointerUp);
    finish(event);
  };

  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', handlePointerUp);
}

/** 管理节点拖拽、连线、框选、平移、缩放和右键菜单定位。 */
export function useCanvasPointerInteractions({
  containerRef,
  maxZoom,
  onConnect,
  onReconnect,
  spacePressed,
  toolMode,
}: UseCanvasPointerInteractionsOptions): CanvasPointerInteractions {
  const store = useFlowStoreApi();
  const [guides, setGuides] = useState<ReadonlyArray<AlignmentGuide>>([]);
  const [contextMenu, setContextMenu] = useState<CanvasContextMenu | null>(null);

  const pointerWorld = useCallback((pointer: Pick<PointerEvent, 'clientX' | 'clientY'>) => {
    const element = containerRef.current;
    if (!element) return null;

    const bounds = element.getBoundingClientRect();
    return screenToWorld(
      {
        x: pointer.clientX - bounds.left,
        y: pointer.clientY - bounds.top,
      },
      store.getState().viewport,
    );
  }, [containerRef, store]);

  const handleNodeDragStart = useCallback((nodeId: string, event: ReactPointerEvent) => {
    if (event.button !== 0) return;

    const dragStart = pointerWorld(event.nativeEvent);
    if (!dragStart) return;

    const initialState = store.getState();
    if (!initialState.selectedNodeIds.has(nodeId)) {
      initialState.selectNodes([nodeId]);
    }

    /** 拖拽全程直接更新节点，结束时再把起始快照压入一次历史。 */
    const initialNodes = structuredClone(store.getState().nodes);
    const initialMetadata = structuredClone(store.getState().metadata);
    const initialDraggedNode = initialNodes.find((node) => node.id === nodeId);
    if (!initialDraggedNode) return;

    const move = (pointerEvent: PointerEvent) => {
      const currentPoint = pointerWorld(pointerEvent);
      if (!currentPoint) return;

      const currentDraggedNode = store.getState().nodes.find((node) => node.id === nodeId);
      if (!currentDraggedNode) return;
      /** 始终从按下时的位置计算总位移，使指针越过吸附阈值后能立即脱离。 */
      const rawPosition = {
        x: Math.round(initialDraggedNode.position.x + currentPoint.x - dragStart.x),
        y: Math.round(initialDraggedNode.position.y + currentPoint.y - dragStart.y),
      };
      store.getState().moveSelected({
        x: rawPosition.x - currentDraggedNode.position.x,
        y: rawPosition.y - currentDraggedNode.position.y,
      });

      const currentState = store.getState();
      if (pointerEvent.altKey || currentState.selectedNodeIds.size !== 1) {
        setGuides([]);
        return;
      }

      const movingNode = currentState.nodes.find(
        (node) => currentState.selectedNodeIds.has(node.id),
      );
      if (!movingNode) return;

      /** 屏幕上的 3px 吸附容差需要按当前倍率换算为世界坐标。 */
      const snapResult = snapNode(
        movingNode,
        currentState.nodes.filter((node) => node.id !== movingNode.id),
        3 / currentState.viewport.zoom,
      );
      const snappedPosition = {
        x: Math.round(snapResult.position.x),
        y: Math.round(snapResult.position.y),
      };
      currentState.moveSelected({
        x: snappedPosition.x - movingNode.position.x,
        y: snappedPosition.y - movingNode.position.y,
      });
      setGuides(snapResult.guides);
    };

    const finish = () => {
      setGuides([]);
      store.setState((currentState) => ({
        future: [],
        historyGroup: null,
        past: [
          ...currentState.past.slice(-99),
          {
            edges: structuredClone(currentState.edges),
            metadata: initialMetadata,
            nodes: initialNodes,
          },
        ],
      }));
    };

    bindPointerGesture(move, finish);
  }, [pointerWorld, store]);

  const handleConnectionStart = useCallback((
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
    reconnect?: ReconnectTarget,
  ) => {
    event.stopPropagation();
    event.preventDefault();
    store.getState().setConnectionDraft({ nodeId, side, point, ...reconnect });

    const move = (pointerEvent: PointerEvent) => {
      const currentPoint = pointerWorld(pointerEvent);
      if (!currentPoint) return;
      store.getState().setConnectionDraft({
        nodeId,
        side,
        point: currentPoint,
        ...reconnect,
      });
    };

    const finish = (pointerEvent: PointerEvent) => {
      const hitElement = document.elementFromPoint(
        pointerEvent.clientX,
        pointerEvent.clientY,
      );
      const targetNode = hitElement?.closest<HTMLElement>('[data-flow-node-id]');
      const targetAnchor = hitElement?.closest<HTMLElement>('[data-anchor-side]');
      const targetId = targetNode?.dataset.flowNodeId;
      const anchorSide = targetAnchor?.dataset.anchorSide;
      const targetSide = isFlowAnchorSide(anchorSide) ? anchorSide : undefined;

      if (targetId) {
        if (reconnect) {
          onReconnect(reconnect.edgeId, reconnect.endpoint, targetId, targetSide);
        } else {
          onConnect(nodeId, targetId, side, targetSide);
        }
      }

      store.getState().setConnectionDraft(null);
    };

    bindPointerGesture(move, finish);
  }, [onConnect, onReconnect, pointerWorld, store]);

  const handleReconnectStart = useCallback((
    edgeId: string,
    endpoint: ConnectionEndpoint,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => {
    const edge = store.getState().edges.find((candidate) => candidate.id === edgeId);
    if (!edge) return;

    handleConnectionStart(
      edge[endpoint].nodeId,
      side,
      point,
      event,
      { edgeId, endpoint },
    );
  }, [handleConnectionStart, store]);

  const handlePanePointerDown = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    setContextMenu(null);
    if (event.button !== 0) return;

    if (spacePressed || toolMode === 'pan') {
      let previousPoint: FlowPoint = {
        x: event.clientX,
        y: event.clientY,
      };

      const move = (pointerEvent: PointerEvent) => {
        const currentPoint = {
          x: pointerEvent.clientX,
          y: pointerEvent.clientY,
        };
        const state = store.getState();
        state.setViewport({
          ...state.viewport,
          x: state.viewport.x + currentPoint.x - previousPoint.x,
          y: state.viewport.y + currentPoint.y - previousPoint.y,
        });
        previousPoint = currentPoint;
      };

      bindPointerGesture(move, () => undefined);
      return;
    }

    const startPoint = pointerWorld(event.nativeEvent);
    if (!startPoint) return;

    store.getState().setSelectionBox({
      start: startPoint,
      end: startPoint,
    });

    const move = (pointerEvent: PointerEvent) => {
      const endPoint = pointerWorld(pointerEvent);
      if (!endPoint) return;
      store.getState().setSelectionBox({ start: startPoint, end: endPoint });
    };

    const finish = (pointerEvent: PointerEvent) => {
      const endPoint = pointerWorld(pointerEvent);
      if (!endPoint) {
        store.getState().setSelectionBox(null);
        return;
      }

      const selectionRect = rectFromPoints(startPoint, endPoint);
      const selectedIds = store.getState().nodes
        .filter((node) => rectsIntersect(
          selectionRect,
          { ...node.position, ...node.size },
        ))
        .map((node) => node.id);
      const selectionMode = pointerEvent.shiftKey
        ? 'add'
        : pointerEvent.ctrlKey
          ? 'toggle'
          : 'replace';

      store.getState().selectNodes(selectedIds, selectionMode);
      store.getState().setSelectionBox(null);
    };

    bindPointerGesture(move, finish);
  }, [pointerWorld, spacePressed, store, toolMode]);

  const handleWheel = useCallback((event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const viewport = store.getState().viewport;
    /** 只限制放大上限；Number.MIN_VALUE 仅防止浮点下溢为零。 */
    const nextZoom = Math.min(
      maxZoom,
      Math.max(Number.MIN_VALUE, viewport.zoom * Math.exp(-event.deltaY * 0.0015)),
    );

    store.getState().setViewport(zoomAt(
      viewport,
      {
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
      },
      nextZoom,
    ));
  }, [maxZoom, store]);

  const handleContextMenu = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    const world = pointerWorld(event.nativeEvent);
    if (!world) return;

    const bounds = event.currentTarget.getBoundingClientRect();
    /** 主菜单宽度为 192px，并为阴影保留少量安全边距。 */
    const menuX = Math.min(
      event.clientX - bounds.left,
      Math.max(8, bounds.width - 200),
    );
    /** 菜单固定在画布内，避免靠近底部时操作项被裁切。 */
    const menuY = Math.min(
      event.clientY - bounds.top,
      Math.max(8, bounds.height - 260),
    );

    setContextMenu({
      x: menuX,
      y: menuY,
      world,
      submenuSide: menuX > bounds.width - 396 ? 'left' : 'right',
    });
  }, [pointerWorld]);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  return {
    closeContextMenu,
    contextMenu,
    guides,
    handleConnectionStart,
    handleContextMenu,
    handleNodeDragStart,
    handlePanePointerDown,
    handleReconnectStart,
    handleWheel,
  };
}
