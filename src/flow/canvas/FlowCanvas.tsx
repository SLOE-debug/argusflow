import { useRef, useState, type DragEvent as ReactDragEvent } from 'react';

import {
  readFlowNodeKindDragData,
} from '../interaction/dragDrop';
import { FlowCanvasLayers } from './FlowCanvasLayers';
import { FlowCanvasTools, type CanvasToolMode } from './FlowCanvasTools';
import { FlowContextMenu } from './FlowContextMenu';
import { screenToWorld } from '../geometry/geometry';
import { useCanvasKeyboard } from '../interaction/useCanvasKeyboard';
import { useCanvasPointerInteractions } from '../interaction/useCanvasPointerInteractions';
import { useCanvasSize } from '../interaction/useCanvasSize';
import { useFlowStoreApi } from '../store/store';
import type {
  FlowAnchorSide,
  FlowEdgeLabelResolver,
  FlowPoint,
  NodeRegistry,
} from '../types';
import { MAX_CANVAS_ZOOM } from '../viewport/viewport';

type FlowCanvasProps = Readonly<{
  registry: Readonly<NodeRegistry>;
  /** 由业务层提供连线语义，通用画布只负责定位和绘制。 */
  edgeLabelResolver?: FlowEdgeLabelResolver;
  onAddNode: (kind: string, position: FlowPoint) => void;
  /** 在连线落点新建节点，并在同一业务事务内完成连线。 */
  onAddConnectedNode: (
    kind: string,
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => boolean;
  onConnect: (
    source: string,
    target: string,
    sourceSide?: FlowAnchorSide,
    targetSide?: FlowAnchorSide,
  ) => boolean;
  onReconnect: (
    edgeId: string,
    endpoint: 'source' | 'target',
    nodeId: string,
    side?: FlowAnchorSide,
  ) => boolean;
  /** 节点双击由业务编辑器决定是否处理。 */
  onNodeDoubleClick?: (nodeId: string) => void;
  /** 放大到结构容器内部时由业务层切换作用域。 */
  onSemanticZoomIn?: (worldPoint: FlowPoint, nextZoom: number) => boolean;
  /** 缩小离开当前结构时由业务层切换到父作用域。 */
  onSemanticZoomOut?: (nextZoom: number) => boolean;
  /** 业务层可接管删除，以同时维护结构容器拥有的外部文档。 */
  onDeleteSelection?: () => void;
}>;

/** 自研 Flow 画布入口，仅装配交互、渲染图层与顶部浮层工具。 */
export function FlowCanvas({
  registry,
  edgeLabelResolver,
  onAddNode,
  onAddConnectedNode,
  onConnect,
  onReconnect,
  onNodeDoubleClick,
  onSemanticZoomIn,
  onSemanticZoomOut,
  onDeleteSelection,
}: FlowCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const store = useFlowStoreApi();
  const [toolMode, setToolMode] = useState<CanvasToolMode>('select');
  const canvasSize = useCanvasSize(containerRef);
  const spacePressed = useCanvasKeyboard(
    registry,
    onNodeDoubleClick,
    onDeleteSelection,
  );
  const interactions = useCanvasPointerInteractions({
    containerRef,
    maxZoom: MAX_CANVAS_ZOOM,
    onConnect,
    onReconnect,
    spacePressed,
    toolMode,
    onSemanticZoomIn,
    onSemanticZoomOut,
  });
  /** 空格与平移工具都必须覆盖节点自身的拖拽手势。 */
  const panActive = spacePressed || toolMode === 'pan';
  const cursorClassName = panActive
    ? 'cursor-grab'
    : 'cursor-crosshair';
  /** 始终声明画布为可放置区域；节点注册键在实际 Drop 时再进行严格校验。 */
  const handleDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };
  /** 将节点库拖放位置转换为世界坐标，并以节点中心对准落点。 */
  const handleDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    const nodeKind = readFlowNodeKindDragData(event.dataTransfer);
    const definition = registry[nodeKind];
    if (!definition) return;

    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const dropPoint = screenToWorld({
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    }, store.getState().viewport);
    /** 拖放位置对应节点中心，避免新节点整体偏向指针右下方。 */
    const position = {
      x: Math.round(dropPoint.x - definition.defaultSize.width / 2),
      y: Math.round(dropPoint.y - definition.defaultSize.height / 2),
    };
    onAddNode(nodeKind, position);
  };

  return (
    <div
      ref={containerRef}
      className={`absolute inset-0 touch-none select-none overflow-hidden bg-white ${cursorClassName}`}
      onContextMenu={interactions.handleContextMenu}
      onDragEnter={handleDragOver}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onPointerDown={interactions.handlePanePointerDown}
    >
      <FlowCanvasLayers
        edgeLabelResolver={edgeLabelResolver}
        guides={interactions.guides}
        onConnectionStart={interactions.handleConnectionStart}
        onDragStart={interactions.handleNodeDragStart}
        onNodeDoubleClick={onNodeDoubleClick}
        onReconnectStart={interactions.handleReconnectStart}
        panActive={panActive}
        registry={registry}
        size={canvasSize}
        toolMode={toolMode}
      />
      <FlowCanvasTools
        canvasSize={canvasSize}
        mode={toolMode}
        onModeChange={setToolMode}
      />
      {interactions.contextMenu ? (
        <FlowContextMenu
          context={interactions.contextMenu}
          nodes={store.getState().nodes}
          onAddNode={onAddNode}
          onAddConnectedNode={onAddConnectedNode}
          onClose={interactions.closeContextMenu}
          onDeleteSelection={onDeleteSelection}
          registry={registry}
        />
      ) : null}
    </div>
  );
}
