import { anchorPoint, pointsBounds, rectsIntersect } from './geometry';
import { getFlowNodeLookup } from './nodeLookup';
import {
  findGridPath,
  isIndexedPathClear,
  type RoutingObstacle,
} from './routingPathfinder';
import { SpatialHash } from './spatialIndex';
import type { FlowAnchorSide, FlowEdge, FlowNode, FlowPoint, FlowRect, RoutedEdge } from './types';

const SIDES: FlowAnchorSide[] = ['top', 'right', 'bottom', 'left'];
const OBSTACLE_GAP = 16;
/** 端点在允许转弯前必须沿锚点法线离开节点的世界像素。 */
const ENDPOINT_CLEARANCE = 14;
export type { RoutingObstacle } from './routingPathfinder';

/** 为批量路由构建一次节点空间索引。 */
export function createRoutingIndex(nodes: ReadonlyArray<FlowNode>): SpatialHash<RoutingObstacle> {
  const index = new SpatialHash<RoutingObstacle>();
  for (const node of nodes) {
    const obstacle = { id: node.id, rect: nodeObstacle(node) };
    index.set(obstacle, obstacle.rect);
  }
  return index;
}

/** 将节点转换为带安全距离的路由障碍物。 */
export function nodeObstacle(node: FlowNode): FlowRect {
  return { x: node.position.x - OBSTACLE_GAP, y: node.position.y - OBSTACLE_GAP, width: node.size.width + OBSTACLE_GAP * 2, height: node.size.height + OBSTACLE_GAP * 2 };
}

/** 为一条边选择锚点并生成避障圆角正交路径。 */
export function routeEdge(edge: FlowEdge, nodes: ReadonlyArray<FlowNode>, previous?: RoutedEdge, sharedIndex?: SpatialHash<RoutingObstacle>): RoutedEdge | null {
  const nodesById = getFlowNodeLookup(nodes);
  const source = nodesById.get(edge.source.nodeId);
  const target = nodesById.get(edge.target.nodeId);
  if (!source || !target) return null;
  const sourceRect = { ...source.position, ...source.size };
  const targetRect = { ...target.position, ...target.size };
  const obstacleIndex = sharedIndex ?? createRoutingIndex(nodes);
  /** 起止节点改由真实边界约束，其他节点仍使用膨胀后的安全区。 */
  const excluded = new Set([source.id, target.id]);
  const endpointRects = [sourceRect, targetRect] as const;
  const candidates = anchorCandidates(edge, sourceRect, targetRect, previous);
  let best: { points: FlowPoint[]; sourceSide: FlowAnchorSide; targetSide: FlowAnchorSide; score: number } | null = null;
  for (const candidate of candidates) {
    const start = anchorPoint(sourceRect, candidate.sourceSide);
    const end = anchorPoint(targetRect, candidate.targetSide);
    const sourceExit = offsetAnchor(start, candidate.sourceSide);
    const targetApproach = offsetAnchor(end, candidate.targetSide);
    const simple = simplifyPoints(orthogonalCandidates(sourceExit, targetApproach).find((points) => (
      isIndexedPathClear(points, obstacleIndex, excluded, endpointRects)
    )) ?? []);
    const gridPoints = simple.length > 0
      ? []
      : findGridPath(
          sourceExit,
          targetApproach,
          obstacleIndex,
          excluded,
          endpointRects,
        );
    const corePoints = simple.length > 0
      ? simple
      : gridPoints;
    if (corePoints.length === 0) continue;
    const points = joinEndpointSegments(
      start,
      end,
      candidate.sourceSide,
      candidate.targetSide,
      corePoints,
    );
    const score = pathLength(points) + (previous && (previous.sourceSide !== candidate.sourceSide || previous.targetSide !== candidate.targetSide) ? 36 : 0);
    if (!best || score < best.score) best = { ...candidate, points, score };
  }
  if (!best) {
    const sourceSide = edge.source.side ?? 'right';
    const targetSide = edge.target.side ?? 'left';
    const start = anchorPoint(sourceRect, sourceSide);
    const end = anchorPoint(targetRect, targetSide);
    const sourceExit = offsetAnchor(start, sourceSide);
    const targetApproach = offsetAnchor(end, targetSide);
    const routingObstacles = routeObstacles(nodes, source.id, target.id);
    const corePoints = shortestObstacleSafePreview(
      sourceExit,
      targetApproach,
      routingObstacles,
    ) ?? findGridPathAgainstRects(sourceExit, targetApproach, routingObstacles);
    if (corePoints.length === 0) return null;
    const points = joinEndpointSegments(
      start,
      end,
      sourceSide,
      targetSide,
      corePoints,
    );
    return { edgeId: edge.id, points, sourceSide, targetSide, path: roundedPath(points), bounds: pointsBounds(points) };
  }
  return { edgeId: edge.id, points: best.points, sourceSide: best.sourceSide, targetSide: best.targetSide, path: roundedPath(best.points), bounds: pointsBounds(best.points) };
}

/**
 * 生成优先使用简单候选的实时预览路径。
 *
 * 精确路径已选定的锚点边会继续沿用；端点变化时重新选择一条避开全部节点
 * 安全区的正交折线，简单候选全部受阻时再执行网格寻路。
 */
export function previewEdgeRoute(
  edge: FlowEdge,
  nodes: ReadonlyArray<FlowNode>,
  previous?: RoutedEdge,
): RoutedEdge | null {
  const nodesById = getFlowNodeLookup(nodes);
  const source = nodesById.get(edge.source.nodeId);
  const target = nodesById.get(edge.target.nodeId);
  if (!source || !target) return null;

  const sourceRect = { ...source.position, ...source.size };
  const targetRect = { ...target.position, ...target.size };
  /** 预览优先保留精确路由已经选定的锚点边。 */
  const sourceSide = edge.source.side ?? previous?.sourceSide ?? 'right';
  const targetSide = edge.target.side ?? previous?.targetSide ?? 'left';
  const start = anchorPoint(sourceRect, sourceSide);
  const end = anchorPoint(targetRect, targetSide);
  const sourceExit = offsetAnchor(start, sourceSide);
  const targetApproach = offsetAnchor(end, targetSide);
  const routingObstacles = routeObstacles(nodes, source.id, target.id);
  const corePoints = shortestObstacleSafePreview(
    sourceExit,
    targetApproach,
    routingObstacles,
  ) ?? findGridPathAgainstRects(sourceExit, targetApproach, routingObstacles);
  if (corePoints.length === 0) return null;
  const points = joinEndpointSegments(
    start,
    end,
    sourceSide,
    targetSide,
    corePoints,
  );

  return {
    edgeId: edge.id,
    points,
    sourceSide,
    targetSide,
    path: roundedPath(points),
    bounds: pointsBounds(points),
  };
}

/** 把正交折线转换为包含二次曲线圆角的 SVG path。 */
export function roundedPath(points: FlowPoint[], radius = 12): string {
  if (points.length === 0) return '';
  if (points.length === 1) return `M ${points[0].x} ${points[0].y}`;
  let path = `M ${points[0].x} ${points[0].y}`;
  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1];
    const current = points[index];
    const next = points[index + 1];
    const incoming = Math.min(radius, distance(previous, current) / 2);
    const outgoing = Math.min(radius, distance(current, next) / 2);
    const before = moveToward(current, previous, incoming);
    const after = moveToward(current, next, outgoing);
    path += ` L ${before.x} ${before.y} Q ${current.x} ${current.y} ${after.x} ${after.y}`;
  }
  const last = points.at(-1)!;
  return `${path} L ${last.x} ${last.y}`;
}

type Candidate = { sourceSide: FlowAnchorSide; targetSide: FlowAnchorSide };

function anchorCandidates(edge: FlowEdge, source: FlowRect, target: FlowRect, previous?: RoutedEdge): Candidate[] {
  if (edge.source.side && edge.target.side) return [{ sourceSide: edge.source.side, targetSide: edge.target.side }];
  const candidates: Candidate[] = [];
  for (const sourceSide of edge.source.side ? [edge.source.side] : SIDES) {
    for (const targetSide of edge.target.side ? [edge.target.side] : SIDES) candidates.push({ sourceSide, targetSide });
  }
  return candidates.sort((a, b) => anchorScore(a, source, target, previous) - anchorScore(b, source, target, previous));
}

function anchorScore(candidate: Candidate, source: FlowRect, target: FlowRect, previous?: RoutedEdge): number {
  const start = anchorPoint(source, candidate.sourceSide);
  const end = anchorPoint(target, candidate.targetSide);
  const directionPenalty = outwardPenalty(start, end, candidate.sourceSide) + outwardPenalty(end, start, candidate.targetSide);
  const hysteresis = previous && (candidate.sourceSide !== previous.sourceSide || candidate.targetSide !== previous.targetSide) ? 36 : 0;
  return Math.abs(start.x - end.x) + Math.abs(start.y - end.y) + directionPenalty + hysteresis;
}

function outwardPenalty(from: FlowPoint, to: FlowPoint, side: FlowAnchorSide): number {
  if (side === 'right' && to.x >= from.x || side === 'left' && to.x <= from.x || side === 'bottom' && to.y >= from.y || side === 'top' && to.y <= from.y) return 0;
  return 160;
}

function orthogonalCandidates(start: FlowPoint, end: FlowPoint): FlowPoint[][] {
  const midX = (start.x + end.x) / 2;
  const midY = (start.y + end.y) / 2;
  return [
    [start, { x: end.x, y: start.y }, end],
    [start, { x: start.x, y: end.y }, end],
    [start, { x: midX, y: start.y }, { x: midX, y: end.y }, end],
    [start, { x: start.x, y: midY }, { x: end.x, y: midY }, end],
  ];
}

/** 将路由主体与源端出口、目标端入口连接，并移除冗余共线点。 */
function joinEndpointSegments(
  start: FlowPoint,
  end: FlowPoint,
  sourceSide: FlowAnchorSide,
  targetSide: FlowAnchorSide,
  corePoints: ReadonlyArray<FlowPoint>,
): FlowPoint[] {
  return simplifyPoints([
    start,
    offsetAnchor(start, sourceSide),
    ...corePoints,
    offsetAnchor(end, targetSide),
    end,
  ]);
}

/** 沿锚点所在边的外法线移动，建立不会贴边转弯的安全出口。 */
function offsetAnchor(
  point: FlowPoint,
  side: FlowAnchorSide,
): FlowPoint {
  switch (side) {
    case 'top':
      return { x: point.x, y: point.y - ENDPOINT_CLEARANCE };
    case 'right':
      return { x: point.x + ENDPOINT_CLEARANCE, y: point.y };
    case 'bottom':
      return { x: point.x, y: point.y + ENDPOINT_CLEARANCE };
    case 'left':
      return { x: point.x - ENDPOINT_CLEARANCE, y: point.y };
  }
}

/** 起止节点使用真实边界，其余节点保留膨胀后的避障安全区。 */
function routeObstacles(
  nodes: ReadonlyArray<FlowNode>,
  sourceId: string,
  targetId: string,
): FlowRect[] {
  return nodes.map((node) => (
    node.id === sourceId || node.id === targetId
      ? { ...node.position, ...node.size }
      : nodeObstacle(node)
  ));
}

/** 实时预览从全部简单候选中选择不穿过任何节点安全区的最短路径。 */
function shortestObstacleSafePreview(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: ReadonlyArray<FlowRect>,
): FlowPoint[] | null {
  const outerBounds = getOuterObstacleBounds(obstacles);
  const perimeterCandidates = outerBounds
    ? [
        [start, { x: start.x, y: outerBounds.top }, { x: end.x, y: outerBounds.top }, end],
        [start, { x: start.x, y: outerBounds.bottom }, { x: end.x, y: outerBounds.bottom }, end],
        [start, { x: outerBounds.left, y: start.y }, { x: outerBounds.left, y: end.y }, end],
        [start, { x: outerBounds.right, y: start.y }, { x: outerBounds.right, y: end.y }, end],
      ]
    : [];
  const candidates = [
    ...orthogonalCandidates(start, end),
    ...perimeterCandidates,
  ].map(simplifyPoints);
  const clearCandidates = candidates.filter((points) => (
    clearPathAgainstRects(points, obstacles)
  ));
  if (clearCandidates.length === 0) return null;
  return clearCandidates.reduce((best, candidate) => (
      pathLength(candidate) < pathLength(best) ? candidate : best
    ));
}

/** 为实时预览的矩形集合建立临时索引，并在简单路线失败时执行安全寻路。 */
function findGridPathAgainstRects(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: ReadonlyArray<FlowRect>,
): FlowPoint[] {
  const obstacleIndex = new SpatialHash<RoutingObstacle>();
  obstacles.forEach((rect, index) => {
    const obstacle = { id: `preview-${index}`, rect };
    obstacleIndex.set(obstacle, rect);
  });
  return findGridPath(start, end, obstacleIndex, new Set());
}

/** 计算包围全部节点安全区的四条外部绕行基线。 */
function getOuterObstacleBounds(
  obstacles: ReadonlyArray<FlowRect>,
): Readonly<{ left: number; right: number; top: number; bottom: number }> | null {
  if (obstacles.length === 0) return null;

  return {
    left: Math.min(...obstacles.map((obstacle) => obstacle.x)) - ENDPOINT_CLEARANCE,
    right: Math.max(...obstacles.map((obstacle) => obstacle.x + obstacle.width)) + ENDPOINT_CLEARANCE,
    top: Math.min(...obstacles.map((obstacle) => obstacle.y)) - ENDPOINT_CLEARANCE,
    bottom: Math.max(...obstacles.map((obstacle) => obstacle.y + obstacle.height)) + ENDPOINT_CLEARANCE,
  };
}

/** 检查折线路径是否避开给定矩形集合。 */
function clearPathAgainstRects(
  points: ReadonlyArray<FlowPoint>,
  obstacles: ReadonlyArray<FlowRect>,
): boolean {
  for (let index = 1; index < points.length; index += 1) {
    const segment = segmentRect(points[index - 1], points[index]);
    if (obstacles.some((obstacle) => rectsIntersect(segment, obstacle))) {
      return false;
    }
  }
  return true;
}

/** 把水平或垂直线段转换为可参与相交检测的最小矩形。 */
function segmentRect(a: FlowPoint, b: FlowPoint): FlowRect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.max(1, Math.abs(a.x - b.x)),
    height: Math.max(1, Math.abs(a.y - b.y)),
  };
}

function simplifyPoints(points: FlowPoint[]): FlowPoint[] {
  const unique = points.filter((point, index) => index === 0 || point.x !== points[index - 1].x || point.y !== points[index - 1].y);
  return unique.filter((point, index) => {
    if (index === 0 || index === unique.length - 1) return true;
    const previous = unique[index - 1];
    const next = unique[index + 1];
    return !((previous.x === point.x && point.x === next.x) || (previous.y === point.y && point.y === next.y));
  });
}

const pathLength = (points: FlowPoint[]): number => points.slice(1).reduce((sum, point, index) => sum + distance(points[index], point), 0);
const distance = (a: FlowPoint, b: FlowPoint): number => Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
const moveToward = (from: FlowPoint, to: FlowPoint, amount: number): FlowPoint => {
  const length = Math.hypot(to.x - from.x, to.y - from.y) || 1;
  return { x: from.x + (to.x - from.x) / length * amount, y: from.y + (to.y - from.y) / length * amount };
};
