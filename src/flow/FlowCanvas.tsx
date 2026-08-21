import { useRef, useState, type DragEvent as ReactDragEvent } from 'react';

import { FLOW_NODE_KIND_DRAG_TYPE } from './dragDrop';
import { FlowCanvasLayers } from './FlowCanvasLayers';
import { FlowCanvasTools, type CanvasToolMode } from './FlowCanvasTools';
import { FlowContextMenu } from './FlowContextMenu';
import { screenToWorld } from './geometry';
import { useCanvasKeyboard } from './useCanvasKeyboard';
import { useCanvasPointerInteractions } from './useCanvasPointerInteractions';
import { useCanvasSize } from './useCanvasSize';
import { useFlowStore } from './store';
import type { FlowAnchorSide, FlowPoint, NodeRegistry } from './types';

type FlowCanvasProps = Readonly<{
  registry: Readonly<NodeRegistry>;
  onAddNode: (kind: string, position: FlowPoint) => void;
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
  onConnect,
  onReconnect,
}: FlowCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [toolMode, setToolMode] = useState<CanvasToolMode>('select');
  const nodes = useFlowStore((state) => state.nodes);
  const viewport = useFlowStore((state) => state.viewport);
  const selectionBox = useFlowStore((state) => state.selectionBox);
  const connectionDraft = useFlowStore((state) => state.connectionDraft);
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
  const cursorClassName = spacePressed || toolMode === 'pan'
    ? 'cursor-grab'
    : 'cursor-crosshair';
  /** 仅允许画布注册的节点拖放数据触发浏览器 Drop。 */
  const handleDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!event.dataTransfer.types.includes(FLOW_NODE_KIND_DRAG_TYPE)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };
  /** 将节点库拖放位置转换为世界坐标，并以节点中心对准落点。 */
  const handleDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    const nodeKind = event.dataTransfer.getData(FLOW_NODE_KIND_DRAG_TYPE);
    const definition = registry[nodeKind];
    if (!definition) return;

    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const dropPoint = screenToWorld({
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    }, viewport);
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
      className={`absolute inset-0 touch-none overflow-hidden bg-white ${cursorClassName}`}
      onContextMenu={interactions.handleContextMenu}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onPointerDown={interactions.handlePanePointerDown}
      onWheel={interactions.handleWheel}
    >
      <FlowCanvasLayers
        connectionDraft={connectionDraft}
        guides={interactions.guides}
        nodes={nodes}
        onConnectionStart={interactions.handleConnectionStart}
        onDragStart={interactions.handleNodeDragStart}
        onReconnectStart={interactions.handleReconnectStart}
        registry={registry}
        selectionBox={selectionBox}
        size={canvasSize}
        toolMode={toolMode}
        viewport={viewport}
      />
      <FlowCanvasTools
        mode={toolMode}
        onModeChange={setToolMode}
      />
      {interactions.contextMenu ? (
        <FlowContextMenu
          context={interactions.contextMenu}
          nodes={nodes}
          onAddNode={onAddNode}
          onClose={interactions.closeContextMenu}
          registry={registry}
        />
      ) : null}
    </div>
  );
}
