import { useRef, useState, type DragEvent as ReactDragEvent } from 'react';

import {
  readFlowNodeKindDragData,
} from './dragDrop';
import { FlowCanvasLayers } from './FlowCanvasLayers';
import { FlowCanvasTools, type CanvasToolMode } from './FlowCanvasTools';
import { FlowContextMenu } from './FlowContextMenu';
import { screenToWorld } from './geometry';
import { useCanvasKeyboard } from './useCanvasKeyboard';
import { useCanvasPointerInteractions } from './useCanvasPointerInteractions';
import { useCanvasSize } from './useCanvasSize';
import { useFlowStoreApi } from './store';
import type { FlowAnchorSide, FlowPoint, NodeRegistry } from './types';

type FlowCanvasProps = Readonly<{
  registry: Readonly<NodeRegistry>;
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
}>;

/** 画布允许的最大放大倍率；缩小不设置业务下限。 */
export const MAX_CANVAS_ZOOM = 2.5;

/** 自研 Flow 画布入口，仅装配交互、渲染图层与顶部浮层工具。 */
export function FlowCanvas({
  registry,
  onAddNode,
  onAddConnectedNode,
  onConnect,
  onReconnect,
}: FlowCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const store = useFlowStoreApi();
  const [toolMode, setToolMode] = useState<CanvasToolMode>('select');
  const canvasSize = useCanvasSize(containerRef);
  const spacePressed = useCanvasKeyboard(registry);
  const interactions = useCanvasPointerInteractions({
    containerRef,
    maxZoom: MAX_CANVAS_ZOOM,
    onConnect,
    onReconnect,
    spacePressed,
    toolMode,
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
      onWheel={interactions.handleWheel}
    >
      <FlowCanvasLayers
        guides={interactions.guides}
        onConnectionStart={interactions.handleConnectionStart}
        onDragStart={interactions.handleNodeDragStart}
        onReconnectStart={interactions.handleReconnectStart}
        panActive={panActive}
        registry={registry}
        size={canvasSize}
        toolMode={toolMode}
      />
      <FlowCanvasTools
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
          registry={registry}
        />
      ) : null}
    </div>
  );
}
