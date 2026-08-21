import type { PointerEvent as ReactPointerEvent } from 'react';

import { anchorPoint, isRectVisible } from './geometry';
import { useFlowStore } from './store';
import type {
  FlowAnchorSide,
  FlowEdge,
  FlowNode,
  FlowPoint,
  RoutedEdge,
  ViewportTransform,
} from './types';
import { useEdgeRoutes } from './useEdgeRoutes';

type FlowEdgesProps = Readonly<{
  width: number;
  height: number;
  onReconnectStart: (
    edgeId: string,
    endpoint: 'source' | 'target',
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
}>;

type VisibleRoute = Readonly<{
  edge: FlowEdge;
  route: RoutedEdge;
}>;

/** 运行粒子的固定数量；错开启动以表现沿线流动。 */
const ACTIVE_PARTICLES = [0, 1, 2, 3] as const;

/** 渲染正交避障连线、透明命中区、分支标签和运行粒子。 */
export function FlowEdges({ width, height, onReconnectStart }: FlowEdgesProps) {
  const nodes = useFlowStore((state) => state.nodes);
  const edges = useFlowStore((state) => state.edges);
  const viewport = useFlowStore((state) => state.viewport);
  const selectedEdgeId = useFlowStore((state) => state.selectedEdgeId);
  const hoveredEdgeId = useFlowStore((state) => state.hoveredEdgeId);
  const activeEdgeIds = useFlowStore((state) => state.activeEdgeIds);
  const selectEdge = useFlowStore((state) => state.selectEdge);
  const setHoveredEdge = useFlowStore((state) => state.setHoveredEdge);
  const routedEdges = useEdgeRoutes(nodes, edges);
  const edgeById = new Map(edges.map((edge) => [edge.id, edge]));
  const visibleRoutes = routedEdges.flatMap((route): VisibleRoute[] => {
    const edge = edgeById.get(route.edgeId);
    return edge && isRectVisible(route.bounds, viewport, width, height)
      ? [{ edge, route }]
      : [];
  });

  return (
    <svg
      className="pointer-events-none absolute inset-0 z-[1] overflow-visible"
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
            interactive={edge.id === selectedEdgeId || edge.id === hoveredEdgeId}
            nodes={nodes}
            onHover={setHoveredEdge}
            onReconnectStart={onReconnectStart}
            onSelect={selectEdge}
            route={route}
            viewport={viewport}
          />
        ))}
      </g>
    </svg>
  );
}

/** 定义所有连线共用的 SVG 箭头。 */
function EdgeMarkerDefinition() {
  return (
    <defs>
      <marker
        id="flow-arrow"
        markerHeight="7"
        markerWidth="7"
        orient="auto-start-reverse"
        refX="9"
        refY="5"
        viewBox="0 0 10 10"
      >
        <path
          d="M 0 0 L 10 5 L 0 10 z"
          fill="#7c91aa"
        />
      </marker>
    </defs>
  );
}

type FlowEdgePathProps = Readonly<{
  active: boolean;
  edge: FlowEdge;
  interactive: boolean;
  nodes: ReadonlyArray<FlowNode>;
  onHover: (edgeId: string | null) => void;
  onReconnectStart: FlowEdgesProps['onReconnectStart'];
  onSelect: (edgeId: string | null) => void;
  route: RoutedEdge;
  viewport: ViewportTransform;
}>;

/** 渲染单条边及其命中区、标签、粒子和重连锚点。 */
function FlowEdgePath({
  active,
  edge,
  interactive,
  nodes,
  onHover,
  onReconnectStart,
  onSelect,
  route,
  viewport,
}: FlowEdgePathProps) {
  const sourceNode = nodes.find((node) => node.id === edge.source.nodeId);
  const targetNode = nodes.find((node) => node.id === edge.target.nodeId);
  if (!sourceNode || !targetNode) return null;

  const sourcePoint = anchorPoint(
    { ...sourceNode.position, ...sourceNode.size },
    route.sourceSide,
  );
  const targetPoint = anchorPoint(
    { ...targetNode.position, ...targetNode.size },
    route.targetSide,
  );
  const branchLabel = readBranchLabel(edge.data);
  const strokeColor = active || interactive ? '#2563eb' : '#7c91aa';
  const strokeWidth = interactive ? 2.3 : 1.7;
  const selectCurrentEdge = (event: ReactPointerEvent<SVGGElement>) => {
    event.stopPropagation();
    onSelect(edge.id);
  };
  const hoverCurrentEdge = () => onHover(edge.id);
  const clearHoveredEdge = () => onHover(null);

  return (
    <g
      className="[pointer-events:stroke]"
      onPointerDown={selectCurrentEdge}
      onPointerEnter={hoverCurrentEdge}
      onPointerLeave={clearHoveredEdge}
    >
      <path
        d={route.path}
        fill="none"
        stroke="transparent"
        strokeWidth="14"
        vectorEffect="non-scaling-stroke"
      />
      <path
        id={`path-${edge.id}`}
        d={route.path}
        fill="none"
        markerEnd="url(#flow-arrow)"
        stroke={strokeColor}
        strokeWidth={strokeWidth}
        style={active
          ? { filter: 'drop-shadow(0 0 4px rgba(59,130,246,.7))' }
          : undefined}
        vectorEffect="non-scaling-stroke"
      />
      {branchLabel ? (
        <EdgeBranchLabel
          label={branchLabel}
          route={route}
        />
      ) : null}
      {active ? (
        <ActiveEdgeParticles path={route.path} />
      ) : null}
      {interactive ? (
        <ReconnectAnchors
          edgeId={edge.id}
          onReconnectStart={onReconnectStart}
          sourcePoint={sourcePoint}
          sourceSide={route.sourceSide}
          targetPoint={targetPoint}
          targetSide={route.targetSide}
          zoom={viewport.zoom}
        />
      ) : null}
    </g>
  );
}

/** 从未知的业务边数据中安全读取可显示的分支文本。 */
function readBranchLabel(data: unknown): string | null {
  if (typeof data !== 'object' || data === null || !('branch' in data)) return null;
  if (data.branch === 'true') return '满足条件';
  if (data.branch === 'false') return '不满足条件';
  return null;
}

/** 将分支文本沿连线路径居中显示。 */
function EdgeBranchLabel({
  label,
  route,
}: Readonly<{ label: string; route: RoutedEdge }>) {
  const sourcePoint = route.points[0];
  const isPositiveBranch = label === '满足条件';
  /** 正分支在线上方居中，负分支在竖线左侧保持水平可读。 */
  const position = isPositiveBranch
    ? { x: sourcePoint.x + 76, y: sourcePoint.y - 10, anchor: 'middle' as const }
    : { x: sourcePoint.x - 12, y: sourcePoint.y + 58, anchor: 'end' as const };

  return (
    <text
      fill={label === '满足条件' ? '#16a34a' : '#ef4444'}
      fontSize="11"
      fontWeight="600"
      paintOrder="stroke"
      stroke="#ffffff"
      strokeWidth="4"
      textAnchor={position.anchor}
      x={position.x}
      y={position.y}
    >
      {label}
    </text>
  );
}

/** 绘制一次性沿边运动的运行态粒子。 */
function ActiveEdgeParticles({ path }: Readonly<{ path: string }>) {
  return ACTIVE_PARTICLES.map((particle) => (
    <circle
      key={particle}
      className="motion-reduce:hidden"
      fill="#60a5fa"
      r="3.5"
      style={{
        animationDelay: `${particle * 120}ms`,
        filter: 'drop-shadow(0 0 4px #3b82f6)',
      }}
    >
      <animateMotion
        dur="900ms"
        path={path}
        repeatCount="1"
      />
    </circle>
  ));
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
        onPointerDown={reconnectSource}
        point={sourcePoint}
        zoom={zoom}
      />
      <ReconnectAnchor
        onPointerDown={reconnectTarget}
        point={targetPoint}
        zoom={zoom}
      />
    </>
  );
}

type ReconnectAnchorProps = Readonly<{
  onPointerDown: (event: ReactPointerEvent<SVGCircleElement>) => void;
  point: FlowPoint;
  zoom: number;
}>;

/** 渲染保持屏幕尺寸不变的单个重连锚点。 */
function ReconnectAnchor({ onPointerDown, point, zoom }: ReconnectAnchorProps) {
  return (
    <circle
      className="pointer-events-auto cursor-crosshair"
      cx={point.x}
      cy={point.y}
      fill="#fff"
      r={6 / zoom}
      stroke="#2563eb"
      strokeWidth="2"
      vectorEffect="non-scaling-stroke"
      onPointerDown={onPointerDown}
    />
  );
}
