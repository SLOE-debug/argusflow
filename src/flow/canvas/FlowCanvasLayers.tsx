import type { PointerEvent as ReactPointerEvent } from 'react';

import { anchorPoint, isRectVisible, rectFromPoints } from '../geometry/geometry';
import { FlowEdges } from './FlowEdges';
import { FlowNodeView } from './FlowNodeView';
import { findFlowNode } from '../selection/nodeLookup';
import type { CanvasToolMode } from './FlowCanvasTools';
import type { AlignmentGuide } from '../geometry/snapping';
import { useFlowStore, type ConnectionDraft, type SelectionBox } from '../store/store';
import type {
  FlowAnchorSide,
  FlowEdgeLabelResolver,
  FlowNode,
  FlowPoint,
  NodeRegistry,
  ViewportTransform,
} from '../types';
import type { CanvasSize } from '../interaction/useCanvasSize';

type FlowCanvasLayersProps = Readonly<{
  edgeLabelResolver?: FlowEdgeLabelResolver;
  guides: ReadonlyArray<AlignmentGuide>;
  onConnectionStart: (
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  onDragStart: (nodeId: string, event: ReactPointerEvent) => void;
  /** 转发业务层节点双击命令。 */
  onNodeDoubleClick?: (nodeId: string) => void;
  onReconnectStart: (
    edgeId: string,
    endpoint: 'source' | 'target',
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  /** 空格或平移工具是否正在覆盖节点交互。 */
  panActive: boolean;
  registry: Readonly<NodeRegistry>;
  size: CanvasSize;
  toolMode: CanvasToolMode;
}>;

/** 框选覆盖层样式。 */
const SELECTION_OVERLAY_CLASS_NAME = [
  'pointer-events-none absolute rounded-md border border-dashed',
  'border-blue-600/80 bg-blue-500/10',
].join(' ');

/** 随视口变化的点阵背景样式。 */
const CANVAS_GRID_CLASS_NAME = [
  'pointer-events-none absolute inset-0',
  'bg-[linear-gradient(to_right,#edf1f6_1px,transparent_1px),linear-gradient(to_bottom,#edf1f6_1px,transparent_1px)]',
].join(' ');

/** 节点吸附参考线的基础样式。 */
const ALIGNMENT_GUIDE_CLASS_NAME = [
  'pointer-events-none absolute z-20',
].join(' ');

/** 装配画布网格、连线和随世界坐标变换的节点交互图层。 */
export function FlowCanvasLayers({
  edgeLabelResolver,
  guides,
  onConnectionStart,
  onDragStart,
  onNodeDoubleClick,
  onReconnectStart,
  panActive,
  registry,
  size,
  toolMode,
}: FlowCanvasLayersProps) {
  const nodes = useFlowStore((state) => state.nodes);
  const viewport = useFlowStore((state) => state.viewport);
  const selectionBox = useFlowStore((state) => state.selectionBox);
  const connectionDraft = useFlowStore((state) => state.connectionDraft);
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
        edgeLabelResolver={edgeLabelResolver}
        height={size.height}
        onReconnectStart={onReconnectStart}
        panActive={panActive}
        width={size.width}
      />
      <div
        className="pointer-events-none absolute inset-0 z-10 origin-top-left"
        style={{ transform: worldTransform }}
      >
        {visibleNodes.map((node) => (
          <FlowNodeView
            key={node.id}
            nodeId={node.id}
            onConnectionStart={onConnectionStart}
            onDragStart={onDragStart}
            onDoubleClick={onNodeDoubleClick}
            panActive={panActive}
            registry={registry}
            toolMode={toolMode}
          />
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

/** 绘制 WinForms 式动态吸附线，仅保留节点间的紫色线段。 */
function AlignmentGuides({
  guides,
}: Readonly<{ guides: ReadonlyArray<AlignmentGuide> }>) {
  return guides.map((guide, index) => {
    const x2 = guide.axis === 'x' ? guide.value : guide.end;
    const y2 = guide.axis === 'x' ? guide.end : guide.value;

    return (
      <svg
        key={`${guide.axis}-${guide.value}-${guide.kind}-${index}`}
        className={`${ALIGNMENT_GUIDE_CLASS_NAME} top-0 left-0 size-px overflow-visible`}
      >
        <line
          x1={guide.axis === 'x' ? guide.value : guide.start}
          x2={x2}
          y1={guide.axis === 'x' ? guide.start : guide.value}
          y2={y2}
          stroke="#a855f7"
          strokeWidth="1.5"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
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

  const sourceNode = findFlowNode(nodes, connectionDraft.nodeId);
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
    <svg className="pointer-events-none absolute top-0 left-0 z-20 size-px overflow-visible">
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
