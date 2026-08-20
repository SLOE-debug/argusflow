import { useRef } from 'react';

import { FlowCanvasLayers } from './FlowCanvasLayers';
import { FlowCanvasTools } from './FlowCanvasTools';
import { FlowContextMenu } from './FlowContextMenu';
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

/** 自研 Flow 画布入口，仅装配交互、渲染图层与浮层工具。 */
export function FlowCanvas({
  registry,
  onAddNode,
  onConnect,
  onReconnect,
}: FlowCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
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
  });
  const cursorClassName = spacePressed ? 'cursor-grab' : 'cursor-crosshair';

  return (
    <div
      ref={containerRef}
      className={`absolute inset-0 touch-none overflow-hidden bg-[#eaf0f7] ${cursorClassName}`}
      onContextMenu={interactions.handleContextMenu}
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
        viewport={viewport}
      />
      <FlowCanvasTools maxZoom={MAX_CANVAS_ZOOM} />
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
