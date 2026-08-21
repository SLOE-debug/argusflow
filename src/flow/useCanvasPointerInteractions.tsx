import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type WheelEvent as ReactWheelEvent,
} from 'react';

import { rectFromPoints, rectsIntersect, screenToWorld, zoomAt } from './geometry';
import type { CanvasToolMode } from './FlowCanvasTools';
import { findFlowNode } from './nodeLookup';
import {
  bindPointerGesture,
  createAnimationFrameCoalescer,
} from './pointerGesture';
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
  /** 从节点连线落到空白处时，保留待完成的起点。 */
  pendingConnection?: PendingNodeConnection;
}>;

/** 等待通过新建节点完成的连线起点。 */
export type PendingNodeConnection = Readonly<{
  sourceNodeId: string;
  sourceSide: FlowAnchorSide;
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
  /** 同一动画帧内累计的滚轮垂直增量。 */
  const wheelDelta = useRef(0);
  /** 最后一次滚轮事件相对画布左上角的坐标。 */
  const wheelPoint = useRef<FlowPoint | null>(null);
  /** 等待应用滚轮缩放的动画帧 ID。 */
  const wheelFrame = useRef<number | null>(null);

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

  /** 在指针落点打开普通画布菜单或待连线的节点菜单。 */
  const openContextMenu = useCallback((
    pointer: Pick<PointerEvent, 'clientX' | 'clientY'>,
    pendingConnection?: PendingNodeConnection,
  ) => {
    const element = containerRef.current;
    const world = pointerWorld(pointer);
    if (!element || !world) return;

    const bounds = element.getBoundingClientRect();
    /** 主菜单宽度为 192px，并为阴影保留少量安全边距。 */
    const menuX = Math.min(
      pointer.clientX - bounds.left,
      Math.max(8, bounds.width - 200),
    );
    /** 菜单固定在画布内，避免靠近底部时操作项被裁切。 */
    const menuY = Math.min(
      pointer.clientY - bounds.top,
      Math.max(8, bounds.height - 260),
    );

    setContextMenu({
      x: menuX,
      y: menuY,
      world,
      submenuSide: menuX > bounds.width - 396 ? 'left' : 'right',
      pendingConnection,
    });
  }, [containerRef, pointerWorld]);

  const handleNodeDragStart = useCallback((nodeId: string, event: ReactPointerEvent) => {
    if (event.button !== 0) return;

    const dragStart = pointerWorld(event.nativeEvent);
    if (!dragStart) return;

    const initialState = store.getState();
    if (!initialState.selectedNodeIds.has(nodeId)) {
      initialState.selectNodes([nodeId]);
    }

    /** 拖拽全程直接更新节点，结束时再把起始快照压入一次历史。 */
    const initialDocument = store.getState();
    const initialNodes = initialDocument.nodes;
    const initialMetadata = initialDocument.metadata;
    const initialEdges = initialDocument.edges;
    const initialDraggedNode = findFlowNode(initialNodes, nodeId);
    if (!initialDraggedNode) return;

    /** 按当前帧最后一个指针位置更新节点与吸附线。 */
    const applyDragFrame = (pointerEvent: PointerEvent) => {
      const currentPoint = pointerWorld(pointerEvent);
      if (!currentPoint) return;

      const currentDraggedNode = findFlowNode(store.getState().nodes, nodeId);
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

      const movingNodeId = currentState.selectedNodeIds.values().next().value;
      const movingNode = movingNodeId
        ? findFlowNode(currentState.nodes, movingNodeId)
        : undefined;
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
    const dragFrames = createAnimationFrameCoalescer(applyDragFrame);

    const finish = (pointerEvent: PointerEvent) => {
      dragFrames.flush(pointerEvent);
      setGuides([]);
      store.setState((currentState) => currentState.nodes === initialNodes
        ? currentState
        : {
            future: [],
            historyGroup: null,
            past: [
              ...currentState.past.slice(-99),
              {
                edges: initialEdges,
                metadata: initialMetadata,
                nodes: initialNodes,
              },
            ],
          });
    };

    bindPointerGesture({
      move: dragFrames.schedule,
      finish,
      cancel: () => {
        dragFrames.cancel();
        setGuides([]);
        store.getState().setNodes(initialNodes, false);
      },
    });
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

    /** 让临时连线端点每帧跟随最后一个指针位置。 */
    const applyConnectionFrame = (pointerEvent: PointerEvent) => {
      const currentPoint = pointerWorld(pointerEvent);
      if (!currentPoint) return;
      store.getState().setConnectionDraft({
        nodeId,
        side,
        point: currentPoint,
        ...reconnect,
      });
    };
    const connectionFrames = createAnimationFrameCoalescer(applyConnectionFrame);

    const finish = (pointerEvent: PointerEvent) => {
      connectionFrames.flush(pointerEvent);
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
      } else if (!reconnect) {
        openContextMenu(pointerEvent, {
          sourceNodeId: nodeId,
          sourceSide: side,
        });
      }

      store.getState().setConnectionDraft(null);
    };

    bindPointerGesture({
      move: connectionFrames.schedule,
      finish,
      cancel: () => {
        connectionFrames.cancel();
        store.getState().setConnectionDraft(null);
      },
    });
  }, [onConnect, onReconnect, openContextMenu, pointerWorld, store]);

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

    /** 框选和平移属于画布手势，禁止浏览器同时创建 SVG/文字原生选区。 */
    event.preventDefault();
    window.getSelection()?.removeAllRanges();

    if (spacePressed || toolMode === 'pan') {
      /** 手势起点的屏幕坐标，用于稳定计算总平移量。 */
      const startPoint: FlowPoint = {
        x: event.clientX,
        y: event.clientY,
      };
      /** 手势开始时的视口，避免合帧后累计误差。 */
      const initialViewport = store.getState().viewport;
      const applyPanFrame = (pointerEvent: PointerEvent) => {
        store.getState().setViewport({
          ...initialViewport,
          x: initialViewport.x + pointerEvent.clientX - startPoint.x,
          y: initialViewport.y + pointerEvent.clientY - startPoint.y,
        });
      };
      const panFrames = createAnimationFrameCoalescer(applyPanFrame);

      bindPointerGesture({
        move: panFrames.schedule,
        finish: panFrames.flush,
        cancel: panFrames.cancel,
      });
      return;
    }

    const startPoint = pointerWorld(event.nativeEvent);
    if (!startPoint) return;

    store.getState().setSelectionBox({
      start: startPoint,
      end: startPoint,
    });

    /** 更新本帧最后一个指针位置对应的框选覆盖层。 */
    const applySelectionFrame = (pointerEvent: PointerEvent) => {
      const endPoint = pointerWorld(pointerEvent);
      if (!endPoint) return;
      store.getState().setSelectionBox({ start: startPoint, end: endPoint });
    };
    const selectionFrames = createAnimationFrameCoalescer(applySelectionFrame);

    const finish = (pointerEvent: PointerEvent) => {
      selectionFrames.flush(pointerEvent);
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

    bindPointerGesture({
      move: selectionFrames.schedule,
      finish,
      cancel: () => {
        selectionFrames.cancel();
        store.getState().setSelectionBox(null);
      },
    });
  }, [pointerWorld, spacePressed, store, toolMode]);

  const handleWheel = useCallback((event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    wheelDelta.current += event.deltaY;
    wheelPoint.current = {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    };
    wheelFrame.current ??= requestAnimationFrame(() => {
      const viewport = store.getState().viewport;
      const screenPoint = wheelPoint.current;
      /** 当前帧累计的滚轮增量会在读取后立即清零。 */
      const deltaY = wheelDelta.current;
      wheelFrame.current = null;
      wheelDelta.current = 0;
      wheelPoint.current = null;
      if (!screenPoint) return;

      /** 只限制放大上限；Number.MIN_VALUE 仅防止浮点下溢为零。 */
      const nextZoom = Math.min(
        maxZoom,
        Math.max(Number.MIN_VALUE, viewport.zoom * Math.exp(-deltaY * 0.0015)),
      );
      store.getState().setViewport(zoomAt(
        viewport,
        screenPoint,
        nextZoom,
      ));
    });
  }, [maxZoom, store]);

  useEffect(() => () => {
    if (wheelFrame.current !== null) cancelAnimationFrame(wheelFrame.current);
  }, []);

  const handleContextMenu = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    openContextMenu(event.nativeEvent);
  }, [openContextMenu]);

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
