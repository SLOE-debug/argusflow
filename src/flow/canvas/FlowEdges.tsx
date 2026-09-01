import { memo, type PointerEvent as ReactPointerEvent } from 'react';

import { anchorPoint, isRectVisible } from '../geometry/geometry';
import { getFlowNodeLookup } from '../selection/nodeLookup';
import { useFlowStore } from '../store/store';
import type {
  FlowAnchorSide,
  FlowEdge,
  FlowEdgeLabel,
  FlowEdgeLabelResolver,
  FlowNode,
  FlowPoint,
  RoutedEdge,
  ViewportTransform,
} from '../types';
import type { FlowCanvasInteractionMode } from './FlowCanvas';
import { useEdgeRoutes } from '../useEdgeRoutes';

type FlowEdgesProps = Readonly<{
  width: number;
  height: number;
  /** 业务层可选的连线标签解析器。 */
  edgeLabelResolver?: FlowEdgeLabelResolver;
  /** 平移手势是否覆盖边选择和重连。 */
  panActive: boolean;
  onReconnectStart: (
    edgeId: string,
    endpoint: 'source' | 'target',
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  interactionMode?: FlowCanvasInteractionMode;
}>;

type VisibleRoute = Readonly<{
  edge: FlowEdge;
  route: RoutedEdge;
}>;

/** 渲染正交避障连线、透明命中区、分支标签和运行脉冲。 */
export const FlowEdges = memo(function FlowEdges({
  width,
  height,
  edgeLabelResolver,
  onReconnectStart,
  panActive,
  interactionMode = 'editable',
}: FlowEdgesProps) {
  const nodes = useFlowStore((state) => state.nodes);
  const edges = useFlowStore((state) => state.edges);
  const viewport = useFlowStore((state) => state.viewport);
  const selectedEdgeId = useFlowStore((state) => state.selectedEdgeId);
  const hoveredEdgeId = useFlowStore((state) => state.hoveredEdgeId);
  const activeEdgeIds = useFlowStore((state) => state.activeEdgeIds);
  const routingInteraction = useFlowStore((state) => state.routingInteraction);
  const selectEdge = useFlowStore((state) => state.selectEdge);
  const setHoveredEdge = useFlowStore((state) => state.setHoveredEdge);
  const routedEdges = useEdgeRoutes(nodes, edges, routingInteraction);
  const edgeById = new Map(edges.map((edge) => [edge.id, edge]));
  const nodesById = getFlowNodeLookup(nodes);
  const visibleRoutes = routedEdges.flatMap((route): VisibleRoute[] => {
    const edge = edgeById.get(route.edgeId);
    return edge && isRectVisible(route.bounds, viewport, width, height)
      ? [{ edge, route }]
      : [];
  });

  return (
    <svg
      className="pointer-events-none absolute inset-0 z-[1] select-none overflow-visible"
      height={height}
      width={width}
    >
      <EdgeMarkerDefinition />
      <g transform={`translate(${viewport.x} ${viewport.y}) scale(${viewport.zoom})`}>
        {visibleRoutes.map(({ edge, route }) => (
          <FlowEdgePath
            key={edge.id}
            active={Boolean(activeEdgeIds[edge.id])}
            edge={edge}
            edgeLabelResolver={edgeLabelResolver}
            hovered={!panActive && edge.id === hoveredEdgeId}
            nodesById={nodesById}
            onHover={setHoveredEdge}
            onReconnectStart={onReconnectStart}
            onSelect={selectEdge}
            panActive={panActive}
            route={route}
            selected={edge.id === selectedEdgeId}
            zoom={viewport.zoom}
            interactionMode={interactionMode}
          />
        ))}
      </g>
    </svg>
  );
});

/** 定义所有连线共用的 SVG 箭头。 */
function EdgeMarkerDefinition() {
  return (
    <defs>
      <marker
        id="flow-arrow"
        markerHeight="7"
        markerUnits="userSpaceOnUse"
        markerWidth="7"
        orient="auto-start-reverse"
        refX="9"
        refY="5"
        viewBox="0 0 10 10"
      >
        <path
          d="M 0 0 L 10 5 L 0 10 z"
          fill="context-stroke"
        />
      </marker>
    </defs>
  );
}

type FlowEdgePathProps = Readonly<{
  active: boolean;
  edge: FlowEdge;
  edgeLabelResolver?: FlowEdgeLabelResolver;
  hovered: boolean;
  nodesById: ReadonlyMap<string, FlowNode>;
  onHover: (edgeId: string | null) => void;
  onReconnectStart: FlowEdgesProps['onReconnectStart'];
  onSelect: (edgeId: string | null) => void;
  panActive: boolean;
  route: RoutedEdge;
  selected: boolean;
  zoom: ViewportTransform['zoom'];
  interactionMode: FlowCanvasInteractionMode;
}>;

/** 渲染单条边及其命中区、标签、运行脉冲和重连锚点。 */
const FlowEdgePath = memo(function FlowEdgePath({
  active,
  edge,
  edgeLabelResolver,
  hovered,
  nodesById,
  onHover,
  onReconnectStart,
  onSelect,
  panActive,
  route,
  selected,
  zoom,
  interactionMode,
}: FlowEdgePathProps) {
  const sourceNode = nodesById.get(edge.source.nodeId);
  const targetNode = nodesById.get(edge.target.nodeId);
  if (!sourceNode || !targetNode) return null;

  const sourcePoint = anchorPoint(
    { ...sourceNode.position, ...sourceNode.size },
    route.sourceSide,
  );
  const targetPoint = anchorPoint(
    { ...targetNode.position, ...targetNode.size },
    route.targetSide,
  );
  const branchLabel = edgeLabelResolver?.(edge.data) ?? null;
  /** 连线使用独立于节点蓝色的紫/青交互色，避免两种对象状态混淆。 */
  const strokeColor = active
    ? '#2563eb'
    : hovered
      ? '#7c3aed'
      : selected
        ? '#0f766e'
        : '#7c91aa';
  const strokeWidth = hovered || selected ? 2.2 : 1.7;
  const strokeDasharray = hovered ? '6 4' : undefined;
  const selectCurrentEdge = (event: ReactPointerEvent<SVGGElement>) => {
    if (panActive) return;
    event.stopPropagation();
    onSelect(edge.id);
  };
  const hoverCurrentEdge = () => {
    if (!panActive) onHover(edge.id);
  };
  const clearHoveredEdge = () => onHover(null);

  return (
    <g
      onPointerDown={selectCurrentEdge}
      onPointerEnter={hoverCurrentEdge}
      onPointerLeave={clearHoveredEdge}
    >
      <path
        className="[pointer-events:stroke]"
        data-flow-edge-hit={edge.id}
        d={route.path}
        fill="none"
        stroke="transparent"
        strokeWidth="14"
        vectorEffect="non-scaling-stroke"
      />
      <path
        className="pointer-events-none"
        id={`path-${edge.id}`}
        d={route.path}
        fill="none"
        markerEnd="url(#flow-arrow)"
        stroke={strokeColor}
        strokeDasharray={strokeDasharray}
        strokeWidth={strokeWidth}
        style={
          active
            ? { filter: 'drop-shadow(0 0 4px rgba(59,130,246,.7))' }
            : hovered
              ? { filter: 'drop-shadow(0 0 3px rgba(124,58,237,.35))' }
              : undefined
        }
        vectorEffect="non-scaling-stroke"
      />
      {branchLabel ? (
        <EdgeBranchLabel
          edgeId={edge.id}
          presentation={branchLabel}
          route={route}
        />
      ) : null}
      {active ? (
        <ActiveEdgePulse edgeId={edge.id} path={route.path} />
      ) : null}
      {(hovered || selected) && !panActive && interactionMode === 'editable' ? (
        <ReconnectAnchors
          edgeId={edge.id}
          onReconnectStart={onReconnectStart}
          sourcePoint={sourcePoint}
          sourceSide={route.sourceSide}
          targetPoint={targetPoint}
          targetSide={route.targetSide}
          zoom={zoom}
        />
      ) : null}
    </g>
  );
});

type EdgeLabelPosition = Readonly<{
  /** 标签归属点始终落在实际连线上。 */
  marker: FlowPoint;
  /** 文本从归属点向线段外侧留出少量间距。 */
  text: FlowPoint;
  textAnchor: 'middle' | 'end';
}>;

/** 标签需要的最小直线长度，避免文字挤在转角或节点边缘。 */
const MIN_EDGE_LABEL_SEGMENT_LENGTH = 72;

/** 将标签放在真实线段旁，并用圆点标出它所属的连线。 */
function EdgeBranchLabel({
  edgeId,
  presentation,
  route,
}: Readonly<{
  edgeId: string;
  presentation: FlowEdgeLabel;
  route: RoutedEdge;
}>) {
  const position = resolveEdgeLabelPosition(route.points);
  if (!position) return null;

  return (
    <g className="pointer-events-none select-none">
      <circle
        data-flow-edge-label-marker={edgeId}
        cx={position.marker.x}
        cy={position.marker.y}
        fill={presentation.color}
        r="3"
        stroke="#ffffff"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
      />
      <text
        data-flow-edge-label={edgeId}
        fill={presentation.color}
        fontSize="11"
        fontWeight="600"
        paintOrder="stroke"
        stroke="#ffffff"
        strokeLinejoin="round"
        strokeWidth="4"
        textAnchor={position.textAnchor}
        x={position.text.x}
        y={position.text.y}
      >
        {presentation.text}
      </text>
    </g>
  );
}

/** 优先选择靠近起点且足够容纳文字的线段，短路径则使用其中最长的一段。 */
function resolveEdgeLabelPosition(
  points: ReadonlyArray<FlowPoint>,
): EdgeLabelPosition | null {
  /** 正交路由的每一段都保留长度和方向，便于选择清晰的标注位置。 */
  const segments = points.slice(0, -1).flatMap((start, index) => {
    const end = points[index + 1];
    if (!end) return [];
    const width = Math.abs(end.x - start.x);
    const height = Math.abs(end.y - start.y);
    const length = width + height;
    return length > 0 ? [{ start, end, length, horizontal: width >= height }] : [];
  });
  const segment = segments.find(({ length }) => (
    length >= MIN_EDGE_LABEL_SEGMENT_LENGTH
  )) ?? segments.reduce<(typeof segments)[number] | null>((longest, candidate) => (
    !longest || candidate.length > longest.length ? candidate : longest
  ), null);
  if (!segment) return null;

  const marker = {
    x: (segment.start.x + segment.end.x) / 2,
    y: (segment.start.y + segment.end.y) / 2,
  };
  return segment.horizontal
    ? {
        marker,
        text: { x: marker.x, y: marker.y - 9 },
        textAnchor: 'middle',
      }
    : {
        marker,
        text: { x: marker.x - 9, y: marker.y + 4 },
        textAnchor: 'end',
      };
}

/** 绘制从 source 指向 target 的连续电流脉冲。 */
function ActiveEdgePulse({
  edgeId,
  path,
}: Readonly<{ edgeId: string; path: string }>) {
  return (
    <path
      className="pointer-events-none animate-[argus-edge-current_650ms_linear_infinite] motion-reduce:hidden"
      data-flow-edge-runtime={edgeId}
      d={path}
      fill="none"
      stroke="#60a5fa"
      strokeDasharray="2 11"
      strokeLinecap="round"
      strokeWidth="3.2"
      style={{
        filter: [
          'drop-shadow(0 0 3px rgba(59,130,246,.95))',
          'drop-shadow(0 0 7px rgba(96,165,250,.5))',
        ].join(' '),
      }}
      vectorEffect="non-scaling-stroke"
    />
  );
}

type ReconnectAnchorsProps = Readonly<{
  edgeId: string;
  onReconnectStart: FlowEdgesProps['onReconnectStart'];
  sourcePoint: FlowPoint;
  sourceSide: FlowAnchorSide;
  targetPoint: FlowPoint;
  targetSide: FlowAnchorSide;
  zoom: number;
}>;

/** 绘制选中或悬停连线两端的重连锚点。 */
function ReconnectAnchors({
  edgeId,
  onReconnectStart,
  sourcePoint,
  sourceSide,
  targetPoint,
  targetSide,
  zoom,
}: ReconnectAnchorsProps) {
  const reconnectSource = (event: ReactPointerEvent<SVGCircleElement>) => {
    onReconnectStart(edgeId, 'source', sourceSide, sourcePoint, event);
  };
  const reconnectTarget = (event: ReactPointerEvent<SVGCircleElement>) => {
    onReconnectStart(edgeId, 'target', targetSide, targetPoint, event);
  };

  return (
    <>
      <ReconnectAnchor
        endpoint="source"
        onPointerDown={reconnectSource}
        point={sourcePoint}
        zoom={zoom}
      />
      <ReconnectAnchor
        endpoint="target"
        onPointerDown={reconnectTarget}
        point={targetPoint}
        zoom={zoom}
      />
    </>
  );
}

type ReconnectAnchorProps = Readonly<{
  endpoint: 'source' | 'target';
  onPointerDown: (event: ReactPointerEvent<SVGCircleElement>) => void;
  point: FlowPoint;
  zoom: number;
}>;

/** 渲染保持屏幕尺寸不变、可区分源端与目标端的重连锚点。 */
function ReconnectAnchor({
  endpoint,
  onPointerDown,
  point,
  zoom,
}: ReconnectAnchorProps) {
  const tone = endpoint === 'source'
    ? { fill: '#ecfdf5', stroke: '#059669' }
    : { fill: '#fff7ed', stroke: '#ea580c' };

  return (
    <circle
      className="pointer-events-auto cursor-crosshair"
      cx={point.x}
      cy={point.y}
      fill={tone.fill}
      r={5 / zoom}
      stroke={tone.stroke}
      strokeWidth="2"
      vectorEffect="non-scaling-stroke"
      onPointerDown={onPointerDown}
    >
      <title>{endpoint === 'source' ? '连线起点' : '连线终点'}</title>
    </circle>
  );
}
