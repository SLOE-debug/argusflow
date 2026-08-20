import { anchorPoint, pointsBounds, rectsIntersect } from './geometry';
import { SpatialHash } from './spatialIndex';
import type { FlowAnchorSide, FlowEdge, FlowNode, FlowPoint, FlowRect, RoutedEdge } from './types';

const SIDES: FlowAnchorSide[] = ['top', 'right', 'bottom', 'left'];
const OBSTACLE_GAP = 16;
const GRID_SIZE = 20;
const MAX_EXPANSIONS = 8_000;
export type RoutingObstacle = { id: string; rect: FlowRect };

/** 为批量路由构建一次节点空间索引。 */
export function createRoutingIndex(nodes: FlowNode[]): SpatialHash<RoutingObstacle> {
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
export function routeEdge(edge: FlowEdge, nodes: FlowNode[], previous?: RoutedEdge, sharedIndex?: SpatialHash<RoutingObstacle>): RoutedEdge | null {
  const source = nodes.find((node) => node.id === edge.source.nodeId);
  const target = nodes.find((node) => node.id === edge.target.nodeId);
  if (!source || !target) return null;
  const sourceRect = { ...source.position, ...source.size };
  const targetRect = { ...target.position, ...target.size };
  const obstacleIndex = sharedIndex ?? createRoutingIndex(nodes);
  const excluded = new Set([source.id, target.id]);
  const candidates = anchorCandidates(edge, sourceRect, targetRect, previous);
  let best: { points: FlowPoint[]; sourceSide: FlowAnchorSide; targetSide: FlowAnchorSide; score: number } | null = null;
  for (const candidate of candidates) {
    const start = anchorPoint(sourceRect, candidate.sourceSide);
    const end = anchorPoint(targetRect, candidate.targetSide);
    const simple = simplifyPoints(orthogonalCandidates(start, end).find((points) => clearPath(points, obstacleIndex, excluded)) ?? []);
    const points = simple.length > 0 ? simple : findGridPath(start, end, obstacleIndex, excluded);
    if (points.length === 0) continue;
    const score = pathLength(points) + (previous && (previous.sourceSide !== candidate.sourceSide || previous.targetSide !== candidate.targetSide) ? 36 : 0);
    if (!best || score < best.score) best = { ...candidate, points, score };
  }
  if (!best) {
    const sourceSide = edge.source.side ?? 'right';
    const targetSide = edge.target.side ?? 'left';
    const points = [anchorPoint(sourceRect, sourceSide), anchorPoint(targetRect, targetSide)];
    return { edgeId: edge.id, points, sourceSide, targetSide, path: roundedPath(points), bounds: pointsBounds(points) };
  }
  return { edgeId: edge.id, points: best.points, sourceSide: best.sourceSide, targetSide: best.targetSide, path: roundedPath(best.points), bounds: pointsBounds(best.points) };
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

function clearPath(points: FlowPoint[], obstacles: SpatialHash<RoutingObstacle>, excluded: Set<string>): boolean {
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    const segment: FlowRect = { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.max(1, Math.abs(a.x - b.x)), height: Math.max(1, Math.abs(a.y - b.y)) };
    if ([...obstacles.query(segment)].some((obstacle) => !excluded.has(obstacle.id) && rectsIntersect(segment, obstacle.rect))) return false;
  }
  return true;
}

function findGridPath(start: FlowPoint, end: FlowPoint, obstacles: SpatialHash<RoutingObstacle>, excluded: Set<string>): FlowPoint[] {
  const origin = snap(start);
  const goal = snap(end);
  const open = new Map<string, GridNode>([[key(origin), { point: origin, g: 0, f: heuristic(origin, goal), direction: null }]]);
  const closed = new Set<string>();
  const parent = new Map<string, string>();
  let expansions = 0;
  while (open.size > 0 && expansions < MAX_EXPANSIONS) {
    const current = [...open.values()].reduce((best, node) => node.f < best.f ? node : best);
    const currentKey = key(current.point);
    open.delete(currentKey);
    if (currentKey === key(goal)) return simplifyPoints([start, ...reconstruct(parent, currentKey), end]);
    closed.add(currentKey);
    expansions += 1;
    for (const [direction, delta] of DIRECTIONS) {
      const point = { x: current.point.x + delta.x, y: current.point.y + delta.y };
      const pointKey = key(point);
      const pointRect = { ...point, width: 1, height: 1 };
      if (closed.has(pointKey) || [...obstacles.query(pointRect)].some((obstacle) => !excluded.has(obstacle.id) && pointInside(point, obstacle.rect))) continue;
      const turnCost = current.direction && current.direction !== direction ? 18 : 0;
      const g = current.g + GRID_SIZE + turnCost;
      const existing = open.get(pointKey);
      if (!existing || g < existing.g) {
        open.set(pointKey, { point, g, f: g + heuristic(point, goal), direction });
        parent.set(pointKey, currentKey);
      }
    }
  }
  return [];
}

type GridNode = { point: FlowPoint; g: number; f: number; direction: string | null };
const DIRECTIONS: [string, FlowPoint][] = [['r', { x: GRID_SIZE, y: 0 }], ['d', { x: 0, y: GRID_SIZE }], ['l', { x: -GRID_SIZE, y: 0 }], ['u', { x: 0, y: -GRID_SIZE }]];
const snap = (point: FlowPoint): FlowPoint => ({ x: Math.round(point.x / GRID_SIZE) * GRID_SIZE, y: Math.round(point.y / GRID_SIZE) * GRID_SIZE });
const key = (point: FlowPoint): string => `${point.x},${point.y}`;
const heuristic = (a: FlowPoint, b: FlowPoint): number => Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
const pointInside = (point: FlowPoint, rect: FlowRect): boolean => point.x > rect.x && point.x < rect.x + rect.width && point.y > rect.y && point.y < rect.y + rect.height;

function reconstruct(parent: Map<string, string>, goalKey: string): FlowPoint[] {
  const points: FlowPoint[] = [];
  let current: string | undefined = goalKey;
  while (current) {
    const [x, y] = current.split(',').map(Number);
    points.push({ x, y });
    current = parent.get(current);
  }
  return points.reverse();
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
