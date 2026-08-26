import type { IndexedObstacle, ObstacleIndex } from './obstacleIndex';
import {
  isRouteCoreClear,
  isRoutePointBlocked,
  isRouteSegmentClear,
  type RouteCollisionContext,
} from './routeCollision';
import { orthogonalConnectors } from './routeRepair';
import {
  inflateRect,
  manhattanDistance,
  simplifyOrthogonalPoints,
  unionRects,
} from './routingGeometry';
import type { FlowPoint, FlowRect, RoutedEdge } from './types';

/** 局部走廊按这些世界像素逐级扩大，最后一级回退到全局障碍物。 */
const CORRIDOR_EXPANSIONS = [96, 192, 384] as const;
/** 障碍物角点向外移动一像素，保证可见线段不接触禁止边界。 */
const PORTAL_OFFSET = 1;
/** 每次转弯的额外代价，用于稳定偏好少折点路径。 */
const BEND_PENALTY = 20;
/** 单条局部图搜索最多展开的方向状态数。 */
const MAX_EXPANDED_STATES = 20_000;

/** 一次稀疏 OVG 搜索的路径与诊断信息。 */
export type VisibilityRoute = Readonly<{
  /** source escape 到 target escape 的正交主体折点。 */
  points: ReadonlyArray<FlowPoint>;
  /** 当前成功走廊内的普通障碍物数量。 */
  nearbyObstacleCount: number;
  /** 当前可见图的顶点数量。 */
  vertexCount: number;
  /** A* 为该路线展开的方向状态数量。 */
  expandedStates: number;
}>;

/**
 * 在旧路线附近逐级扩大走廊，使用稀疏正交可见图寻找端口间路径。
 *
 * 常见路径只查询 96px 走廊；局部失败后才扩大，最后一次才读取全局障碍物。
 */
export function findLocalOrthogonalRoute(
  start: FlowPoint,
  end: FlowPoint,
  previous: RoutedEdge | undefined,
  sourceRect: FlowRect,
  targetRect: FlowRect,
  obstacleIndex: ObstacleIndex,
  collision: RouteCollisionContext,
): VisibilityRoute | null {
  const baseBounds = previous
    ? unionRects(unionRects(previous.bounds, sourceRect), targetRect)
    : unionRects(sourceRect, targetRect);
  for (const expansion of CORRIDOR_EXPANSIONS) {
    const corridor = inflateRect(baseBounds, expansion);
    const nearby = obstacleIndex.query(corridor).filter((obstacle) => (
      !collision.excludedNodeIds.has(obstacle.nodeId)
    ));
    const route = searchVisibilityGraph(start, end, nearby, collision);
    if (route) return { ...route, nearbyObstacleCount: nearby.length };
  }
  const globalObstacles = obstacleIndex.all().filter((obstacle) => (
    !collision.excludedNodeIds.has(obstacle.nodeId)
  ));
  const globalRoute = searchVisibilityGraph(
    start,
    end,
    globalObstacles,
    collision,
  );
  return globalRoute
    ? { ...globalRoute, nearbyObstacleCount: globalObstacles.length }
    : null;
}

/** 构建 portal 顶点、最近可见邻接边并执行带方向状态的 A*。 */
function searchVisibilityGraph(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: ReadonlyArray<IndexedObstacle>,
  collision: RouteCollisionContext,
): Omit<VisibilityRoute, 'nearbyObstacleCount'> | null {
  for (const connector of orthogonalConnectors(start, end)) {
    if (isRouteCoreClear(connector, collision)) {
      return {
        points: simplifyOrthogonalPoints(connector),
        vertexCount: connector.length,
        expandedStates: 0,
      };
    }
  }

  const graphObstacles = [
    ...obstacles.map((obstacle) => obstacle.rect),
    ...collision.endpointRects,
  ];
  const points = buildGraphPoints(start, end, graphObstacles, collision);
  const startIndex = points.findIndex((point) => samePoint(point, start));
  const endIndex = points.findIndex((point) => samePoint(point, end));
  if (startIndex < 0 || endIndex < 0) return null;
  const adjacency = buildVisibilityAdjacency(points, collision);
  return searchDirectionAwareAStar(
    points,
    adjacency,
    startIndex,
    endIndex,
  );
}

/**
 * 以障碍物外侧 portal 为主顶点，并向四个方向投影到最近障碍物。
 * 最近投影点会与被命中障碍物的同侧 portal 共线，使图保持 O(M) 顶点规模。
 */
function buildGraphPoints(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: ReadonlyArray<FlowRect>,
  collision: RouteCollisionContext,
): ReadonlyArray<FlowPoint> {
  const portals = obstacles.flatMap(obstaclePortals);
  const seeds = [start, end, ...portals];
  const projections = seeds.flatMap((point) => (
    nearestRayProjections(point, obstacles)
  ));
  const candidates: FlowPoint[] = [...seeds, ...projections];
  const seen = new Set<string>();
  return candidates.filter((point) => {
    const key = pointKey(point);
    if (seen.has(key) || isRoutePointBlocked(point, collision)) return false;
    seen.add(key);
    return true;
  });
}

/** 从一个顶点沿四条正交射线取得最近障碍物外侧投影。 */
function nearestRayProjections(
  point: FlowPoint,
  obstacles: ReadonlyArray<FlowRect>,
): ReadonlyArray<FlowPoint> {
  /** 每个方向只保留距离最近的投影，避免生成 O(M²) 顶点。 */
  let left: FlowPoint | null = null;
  let right: FlowPoint | null = null;
  let top: FlowPoint | null = null;
  let bottom: FlowPoint | null = null;
  for (const rect of obstacles) {
    const containsY = point.y >= rect.y && point.y <= rect.y + rect.height;
    const containsX = point.x >= rect.x && point.x <= rect.x + rect.width;
    const leftProjection = { x: rect.x - PORTAL_OFFSET, y: point.y };
    const rightProjection = {
      x: rect.x + rect.width + PORTAL_OFFSET,
      y: point.y,
    };
    const topProjection = { x: point.x, y: rect.y - PORTAL_OFFSET };
    const bottomProjection = {
      x: point.x,
      y: rect.y + rect.height + PORTAL_OFFSET,
    };
    if (containsY && rightProjection.x < point.x) {
      if (!left || rightProjection.x > left.x) left = rightProjection;
    }
    if (containsY && leftProjection.x > point.x) {
      if (!right || leftProjection.x < right.x) right = leftProjection;
    }
    if (containsX && bottomProjection.y < point.y) {
      if (!top || bottomProjection.y > top.y) top = bottomProjection;
    }
    if (containsX && topProjection.y > point.y) {
      if (!bottom || topProjection.y < bottom.y) bottom = topProjection;
    }
  }
  return [left, right, top, bottom].filter(
    (projection): projection is FlowPoint => projection !== null,
  );
}

/** 在每条水平线和垂直线上只连接相邻可见顶点，形成稀疏图。 */
function buildVisibilityAdjacency(
  points: ReadonlyArray<FlowPoint>,
  collision: RouteCollisionContext,
): ReadonlyArray<ReadonlyArray<GraphNeighbor>> {
  const adjacency: GraphNeighbor[][] = points.map(() => []);
  const horizontalBuckets = new Map<number, number[]>();
  const verticalBuckets = new Map<number, number[]>();
  points.forEach((point, index) => {
    pushBucket(horizontalBuckets, point.y, index);
    pushBucket(verticalBuckets, point.x, index);
  });
  for (const indices of horizontalBuckets.values()) {
    indices.sort((a, b) => points[a].x - points[b].x);
    connectVisibleNeighbors(indices, 'horizontal', points, adjacency, collision);
  }
  for (const indices of verticalBuckets.values()) {
    indices.sort((a, b) => points[a].y - points[b].y);
    connectVisibleNeighbors(indices, 'vertical', points, adjacency, collision);
  }
  return adjacency;
}

/** 给同轴排序后的相邻顶点建立双向可见边。 */
function connectVisibleNeighbors(
  indices: ReadonlyArray<number>,
  direction: GraphDirection,
  points: ReadonlyArray<FlowPoint>,
  adjacency: GraphNeighbor[][],
  collision: RouteCollisionContext,
): void {
  for (let index = 1; index < indices.length; index += 1) {
    const previousIndex = indices[index - 1];
    const currentIndex = indices[index];
    if (!isRouteSegmentClear(
      points[previousIndex],
      points[currentIndex],
      collision,
    )) continue;
    const cost = manhattanDistance(points[previousIndex], points[currentIndex]);
    adjacency[previousIndex].push({ vertexIndex: currentIndex, direction, cost });
    adjacency[currentIndex].push({ vertexIndex: previousIndex, direction, cost });
  }
}

/** 在 vertex + incomingDirection 状态空间执行 A*，把转弯成本纳入真实代价。 */
function searchDirectionAwareAStar(
  points: ReadonlyArray<FlowPoint>,
  adjacency: ReadonlyArray<ReadonlyArray<GraphNeighbor>>,
  startIndex: number,
  endIndex: number,
): Omit<VisibilityRoute, 'nearbyObstacleCount'> | null {
  /** 开放列表使用最小堆，避免全局走廊回退时线性查找最低代价状态。 */
  const open = new SearchStateMinHeap();
  open.push({
    vertexIndex: startIndex,
    incomingDirection: null,
    g: 0,
    f: manhattanDistance(points[startIndex], points[endIndex]),
  });
  const bestCosts = new Map<string, number>([[stateKey(startIndex, null), 0]]);
  const parents = new Map<string, string>();
  let expandedStates = 0;

  while (open.size > 0 && expandedStates < MAX_EXPANDED_STATES) {
    const current = open.pop();
    if (!current) break;
    const currentKey = stateKey(
      current.vertexIndex,
      current.incomingDirection,
    );
    if (current.g !== bestCosts.get(currentKey)) continue;
    if (current.vertexIndex === endIndex) {
      return {
        points: reconstructPath(points, parents, currentKey),
        vertexCount: points.length,
        expandedStates,
      };
    }
    expandedStates += 1;
    for (const neighbor of adjacency[current.vertexIndex]) {
      const turnCost = current.incomingDirection
        && current.incomingDirection !== neighbor.direction
        ? BEND_PENALTY
        : 0;
      const g = current.g + neighbor.cost + turnCost;
      const neighborKey = stateKey(neighbor.vertexIndex, neighbor.direction);
      const knownCost = bestCosts.get(neighborKey);
      if (knownCost !== undefined && knownCost <= g) continue;
      bestCosts.set(neighborKey, g);
      parents.set(neighborKey, currentKey);
      open.push({
        vertexIndex: neighbor.vertexIndex,
        incomingDirection: neighbor.direction,
        g,
        f: g + manhattanDistance(points[neighbor.vertexIndex], points[endIndex]),
      });
    }
  }
  return null;
}

/** 从方向状态父链还原并简化折线路径。 */
function reconstructPath(
  points: ReadonlyArray<FlowPoint>,
  parents: ReadonlyMap<string, string>,
  endKey: string,
): ReadonlyArray<FlowPoint> {
  const reversed: FlowPoint[] = [];
  let currentKey: string | undefined = endKey;
  while (currentKey) {
    const [vertexIndex] = currentKey.split(':');
    reversed.push(points[Number(vertexIndex)]);
    currentKey = parents.get(currentKey);
  }
  return simplifyOrthogonalPoints(reversed.reverse());
}

/** 为膨胀矩形生成四个严格位于禁止边界外侧的 portal。 */
function obstaclePortals(rect: FlowRect): ReadonlyArray<FlowPoint> {
  const left = rect.x - PORTAL_OFFSET;
  const right = rect.x + rect.width + PORTAL_OFFSET;
  const top = rect.y - PORTAL_OFFSET;
  const bottom = rect.y + rect.height + PORTAL_OFFSET;
  return [
    { x: left, y: top },
    { x: right, y: top },
    { x: left, y: bottom },
    { x: right, y: bottom },
  ];
}

type GraphDirection = 'horizontal' | 'vertical';

type GraphNeighbor = Readonly<{
  vertexIndex: number;
  direction: GraphDirection;
  cost: number;
}>;

type SearchState = Readonly<{
  vertexIndex: number;
  incomingDirection: GraphDirection | null;
  g: number;
  f: number;
}>;

/** OVG A* 专用最小堆，按预计总代价 f 维护开放状态。 */
class SearchStateMinHeap {
  /** 按 f 值维持堆序的方向状态。 */
  private readonly states: SearchState[] = [];

  /** 当前开放状态数量。 */
  public get size(): number {
    return this.states.length;
  }

  /** 插入状态并向上恢复最小堆。 */
  public push(state: SearchState): void {
    this.states.push(state);
    let index = this.states.length - 1;
    while (index > 0) {
      const parentIndex = Math.floor((index - 1) / 2);
      if (this.states[parentIndex].f <= this.states[index].f) break;
      [this.states[parentIndex], this.states[index]] = [
        this.states[index],
        this.states[parentIndex],
      ];
      index = parentIndex;
    }
  }

  /** 取出最低 f 状态并向下恢复最小堆。 */
  public pop(): SearchState | undefined {
    const first = this.states[0];
    const last = this.states.pop();
    if (!first || !last || this.states.length === 0) return first;
    this.states[0] = last;
    let index = 0;
    while (true) {
      const left = index * 2 + 1;
      const right = left + 1;
      let smallest = index;
      if (
        left < this.states.length
        && this.states[left].f < this.states[smallest].f
      ) smallest = left;
      if (
        right < this.states.length
        && this.states[right].f < this.states[smallest].f
      ) smallest = right;
      if (smallest === index) break;
      [this.states[index], this.states[smallest]] = [
        this.states[smallest],
        this.states[index],
      ];
      index = smallest;
    }
    return first;
  }
}

/** 将索引追加到数值坐标桶。 */
function pushBucket(
  buckets: Map<number, number[]>,
  coordinate: number,
  index: number,
): void {
  const bucket = buckets.get(coordinate) ?? [];
  bucket.push(index);
  buckets.set(coordinate, bucket);
}

/** 点坐标的稳定键。 */
function pointKey(point: FlowPoint): string {
  return `${point.x},${point.y}`;
}

/** 方向状态的稳定键。 */
function stateKey(
  vertexIndex: number,
  direction: GraphDirection | null,
): string {
  return `${vertexIndex}:${direction ?? 'none'}`;
}

/** 精确比较由节点几何派生的坐标点。 */
function samePoint(a: FlowPoint, b: FlowPoint): boolean {
  return a.x === b.x && a.y === b.y;
}
