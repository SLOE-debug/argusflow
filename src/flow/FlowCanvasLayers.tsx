import type { PointerEvent as ReactPointerEvent } from 'react';

import { anchorPoint, isRectVisible, rectFromPoints } from './geometry';
import { FlowEdges } from './FlowEdges';
import { FlowNodeView } from './FlowNodeView';
import type { AlignmentGuide } from './snapping';
import type { ConnectionDraft, SelectionBox } from './store';
import type {
  FlowAnchorSide,
  FlowNode,
  FlowPoint,
  NodeRegistry,
  ViewportTransform,
} from './types';
import type { CanvasSize } from './useCanvasSize';

type FlowCanvasLayersProps = Readonly<{
  connectionDraft: ConnectionDraft | null;
  guides: ReadonlyArray<AlignmentGuide>;
  nodes: ReadonlyArray<FlowNode>;
  onConnectionStart: (
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  onDragStart: (nodeId: string, event: ReactPointerEvent) => void;
  onReconnectStart: (
    edgeId: string,
    endpoint: 'source' | 'target',
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  registry: Readonly<NodeRegistry>;
  selectionBox: SelectionBox | null;
  size: CanvasSize;
  viewport: ViewportTransform;
}>;

/** 框选覆盖层样式。 */
const SELECTION_OVERLAY_CLASS_NAME = [
  'pointer-events-none absolute rounded-[10px] border border-dashed',
  'border-blue-600/80 bg-gradient-to-br from-blue-500/15 to-blue-400/5',
  'shadow-[inset_0_0_0_1px_rgba(255,255,255,.5),0_8px_24px_rgba(37,99,235,.1)]',
].join(' ');

/** 随视口变化的点阵背景样式。 */
const CANVAS_GRID_CLASS_NAME = [
  'pointer-events-none absolute inset-0',
  'bg-[radial-gradient(circle,#b8c5d5_1.1px,transparent_1.2px)]',
].join(' ');

/** 节点吸附参考线的基础样式。 */
const ALIGNMENT_GUIDE_CLASS_NAME = [
  'pointer-events-none absolute z-[100]',
  'bg-pink-500 ring-1 ring-pink-500/10',
].join(' ');

/** 装配画布网格、连线和随世界坐标变换的节点交互图层。 */
export function FlowCanvasLayers({
  connectionDraft,
  guides,
  nodes,
  onConnectionStart,
  onDragStart,
  onReconnectStart,
  registry,
  selectionBox,
  size,
  viewport,
}: FlowCanvasLayersProps) {
  const worldTransform = `translate(${viewport.x}px, ${viewport.y}px) scale(${viewport.zoom})`;
  const visibleNodes = nodes.filter((node) => isRectVisible(
    { ...node.position, ...node.size },
    viewport,
    size.width,
    size.height,
  ));

  return (
    <>
      <CanvasGrid viewport={viewport} />
      <FlowEdges
        height={size.height}
        onReconnectStart={onReconnectStart}
        width={size.width}
      />
      <div
        className="absolute inset-0 origin-top-left"
        style={{ transform: worldTransform }}
      >
        {visibleNodes.map((node) => (
          <div
            key={node.id}
            className="pointer-events-none absolute inset-0"
            data-flow-node-id={node.id}
          >
            <FlowNodeView
              nodeId={node.id}
              onConnectionStart={onConnectionStart}
              onDragStart={onDragStart}
              registry={registry}
            />
          </div>
        ))}
        <AlignmentGuides guides={guides} />
        <SelectionOverlay selectionBox={selectionBox} />
        <ConnectionDraftPath
          connectionDraft={connectionDraft}
          nodes={nodes}
        />
      </div>
    </>
  );
}

/** 绘制随视口平移和缩放的点阵背景。 */
function CanvasGrid({ viewport }: Readonly<{ viewport: ViewportTransform }>) {
  return (
    <div
      className={CANVAS_GRID_CLASS_NAME}
      style={{
        backgroundPosition: `${viewport.x}px ${viewport.y}px`,
        backgroundSize: `${24 * viewport.zoom}px ${24 * viewport.zoom}px`,
      }}
    />
  );
}

/** 绘制节点吸附时跨越可视区域的水平或垂直参考线。 */
function AlignmentGuides({
  guides,
}: Readonly<{ guides: ReadonlyArray<AlignmentGuide> }>) {
  return guides.map((guide, index) => {
    const orientationClassName = guide.axis === 'x'
      ? '-top-[10000px] h-[20000px] w-px'
      : '-left-[10000px] h-px w-[20000px]';
    const positionStyle = guide.axis === 'x'
      ? { left: guide.value }
      : { top: guide.value };

    return (
      <div
        key={`${guide.axis}-${guide.value}-${index}`}
        className={`${ALIGNMENT_GUIDE_CLASS_NAME} ${orientationClassName}`}
        style={positionStyle}
      />
    );
  });
}

/** 绘制当前框选手势覆盖的世界坐标区域。 */
function SelectionOverlay({
  selectionBox,
}: Readonly<{ selectionBox: SelectionBox | null }>) {
  if (!selectionBox) return null;

  const box = rectFromPoints(selectionBox.start, selectionBox.end);
  return (
    <div
      className={SELECTION_OVERLAY_CLASS_NAME}
      style={{
        height: box.height,
        left: box.x,
        top: box.y,
        width: box.width,
      }}
    />
  );
}

type ConnectionDraftPathProps = Readonly<{
  connectionDraft: ConnectionDraft | null;
  nodes: ReadonlyArray<FlowNode>;
}>;

/** 绘制从节点锚点指向当前指针的临时虚线。 */
function ConnectionDraftPath({
  connectionDraft,
  nodes,
}: ConnectionDraftPathProps) {
  if (!connectionDraft) return null;

  const sourceNode = nodes.find((node) => node.id === connectionDraft.nodeId);
  const sourcePoint = sourceNode
    ? anchorPoint(
        { ...sourceNode.position, ...sourceNode.size },
        connectionDraft.side,
      )
    : connectionDraft.point;
  const path = [
    `M ${sourcePoint.x} ${sourcePoint.y}`,
    `L ${connectionDraft.point.x} ${connectionDraft.point.y}`,
  ].join(' ');

  return (
    <svg className="pointer-events-none absolute top-0 left-0 size-px overflow-visible">
      <path
        d={path}
        fill="none"
        stroke="#3b82f6"
        strokeDasharray="6 5"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
