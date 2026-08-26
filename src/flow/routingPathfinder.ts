import { rectsIntersect } from './geometry';
import type { SpatialHash } from './spatialIndex';
import type { FlowPoint, FlowRect } from './types';

/** 路由空间索引保存的节点障碍物。 */
export type RoutingObstacle = Readonly<{
  /** 对应节点或临时障碍物的稳定标识。 */
  id: string;
  /** 连线不得进入或接触的矩形边界。 */
  rect: FlowRect;
}>;

/** 网格步长兼顾画布路径精度与寻路开销。 */
const GRID_SIZE = 20;
/** 单条边允许展开的最大网格状态数。 */
const MAX_EXPANSIONS = 20_000;
/** 转弯成本让同等长度的路径优先减少折点。 */
const TURN_COST = 18;

/**
 * 在空间索引与附加端点矩形之间搜索严格避障的正交路径。
 *
 * 起止点允许不在网格上，函数会为它们选择经过碰撞检查的正交接入段。
 */
export function findGridPath(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: SpatialHash<RoutingObstacle>,
  excluded: ReadonlySet<string>,
  endpointRects: ReadonlyArray<FlowRect> = [],
): FlowPoint[] {
  const origin = snap(start);
  const goal = snap(end);
  const sourceConnector = shortestClearConnector(
    start,
    origin,
    obstacles,
    excluded,
    endpointRects,
  );
  const targetConnector = shortestClearConnector(
    goal,
    end,
    obstacles,
    excluded,
    endpointRects,
  );
  if (!sourceConnector || !targetConnector) return [];

  /** A* 开放集合按预计总代价排序。 */
  const open = new GridNodeMinHeap();
  const originNode = {
    point: origin,
    g: 0,
    f: heuristic(origin, goal),
    direction: null,
  } satisfies GridNode;
  open.push(originNode);
  /** 每个网格点当前已知的最低代价，用于丢弃堆中的过期节点。 */
  const bestCosts = new Map<string, number>([[pointKey(origin), 0]]);
  /** 已确定最低代价的网格点。 */
  const closed = new Set<string>();
  /** 网格路径的父节点索引。 */
  const parent = new Map<string, string>();
  let expansions = 0;

  while (open.size > 0 && expansions < MAX_EXPANSIONS) {
    const current = open.pop();
    if (!current) break;
    const currentKey = pointKey(current.point);
    if (closed.has(currentKey) || current.g !== bestCosts.get(currentKey)) {
      continue;
    }
    if (currentKey === pointKey(goal)) {
      const gridPoints = reconstruct(parent, currentKey);
      return simplifyPoints([
        ...sourceConnector,
        ...gridPoints.slice(1),
        ...targetConnector.slice(1),
      ]);
    }

    closed.add(currentKey);
    expansions += 1;
    for (const [direction, delta] of DIRECTIONS) {
      const point = {
        x: current.point.x + delta.x,
        y: current.point.y + delta.y,
      };
      const nextKey = pointKey(point);
      if (
        closed.has(nextKey)
        || !isIndexedPathClear(
          [current.point, point],
          obstacles,
          excluded,
          endpointRects,
        )
      ) {
        continue;
      }

      const turnCost = current.direction && current.direction !== direction
        ? TURN_COST
        : 0;
      const g = current.g + GRID_SIZE + turnCost;
      const existingCost = bestCosts.get(nextKey);
      if (existingCost !== undefined && existingCost <= g) continue;
      bestCosts.set(nextKey, g);
      open.push({
        point,
        g,
        f: g + heuristic(point, goal),
        direction,
      });
      parent.set(nextKey, currentKey);
    }
  }
  return [];
}

/** 判断整条正交折线是否避开索引障碍物与端点矩形。 */
export function isIndexedPathClear(
  points: ReadonlyArray<FlowPoint>,
  obstacles: SpatialHash<RoutingObstacle>,
  excluded: ReadonlySet<string>,
  endpointRects: ReadonlyArray<FlowRect> = [],
): boolean {
  return points.slice(1).every((point, index) => {
    const segment = segmentRect(points[index], point);
    const crossesIndexedObstacle = [...obstacles.query(segment)].some(
      (obstacle) => (
        !excluded.has(obstacle.id)
        && rectsIntersect(segment, obstacle.rect)
      ),
    );
    return !crossesIndexedObstacle
      && !endpointRects.some((rect) => rectsIntersect(segment, rect));
  });
}

type GridDirection = 'right' | 'down' | 'left' | 'up';

type GridNode = Readonly<{
  point: FlowPoint;
  /** 从原点到当前节点的实际代价。 */
  g: number;
  /** 实际代价与启发式估价之和。 */
  f: number;
  /** 进入当前节点的方向，用于计算转弯成本。 */
  direction: GridDirection | null;
}>;

/** A* 开放集合使用的路由专用最小堆。 */
class GridNodeMinHeap {
  /** 按 f 值维持最小堆顺序的节点。 */
  private readonly nodes: GridNode[] = [];

  /** 当前堆内节点数量。 */
  public get size(): number {
    return this.nodes.length;
  }

  /** 插入新候选并向上恢复堆序。 */
  public push(node: GridNode): void {
    this.nodes.push(node);
    let index = this.nodes.length - 1;
    while (index > 0) {
      const parentIndex = Math.floor((index - 1) / 2);
      if (this.nodes[parentIndex].f <= this.nodes[index].f) break;
      [this.nodes[parentIndex], this.nodes[index]] = [
        this.nodes[index],
        this.nodes[parentIndex],
      ];
      index = parentIndex;
    }
  }

  /** 取出最低 f 值候选并向下恢复堆序。 */
  public pop(): GridNode | undefined {
    const first = this.nodes[0];
    const last = this.nodes.pop();
    if (!first || !last || this.nodes.length === 0) return first;

    this.nodes[0] = last;
    let index = 0;
    while (true) {
      const left = index * 2 + 1;
      const right = left + 1;
      let smallest = index;
      if (
        left < this.nodes.length
        && this.nodes[left].f < this.nodes[smallest].f
      ) {
        smallest = left;
      }
      if (
        right < this.nodes.length
        && this.nodes[right].f < this.nodes[smallest].f
      ) {
        smallest = right;
      }
      if (smallest === index) break;
      [this.nodes[smallest], this.nodes[index]] = [
        this.nodes[index],
        this.nodes[smallest],
      ];
      index = smallest;
    }
    return first;
  }
}

/** 四个正交网格扩展方向。 */
const DIRECTIONS = [
  ['right', { x: GRID_SIZE, y: 0 }],
  ['down', { x: 0, y: GRID_SIZE }],
  ['left', { x: -GRID_SIZE, y: 0 }],
  ['up', { x: 0, y: -GRID_SIZE }],
] as const satisfies ReadonlyArray<readonly [GridDirection, FlowPoint]>;

/** 从非网格端点选择一条经过碰撞检查的最短正交接入路径。 */
function shortestClearConnector(
  start: FlowPoint,
  end: FlowPoint,
  obstacles: SpatialHash<RoutingObstacle>,
  excluded: ReadonlySet<string>,
  endpointRects: ReadonlyArray<FlowRect>,
): FlowPoint[] | null {
  const candidates = [
    [start, { x: end.x, y: start.y }, end],
    [start, { x: start.x, y: end.y }, end],
  ].map(simplifyPoints).filter((points) => (
    isIndexedPathClear(points, obstacles, excluded, endpointRects)
  ));
  if (candidates.length === 0) return null;
  return candidates.reduce((best, candidate) => (
    pathLength(candidate) < pathLength(best) ? candidate : best
  ));
}

/** 将线段规范化为支持零宽或零高的精确碰撞矩形。 */
function segmentRect(a: FlowPoint, b: FlowPoint): FlowRect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

/** 将任意画布点吸附到最近的路由网格。 */
function snap(point: FlowPoint): FlowPoint {
  return {
    x: Math.round(point.x / GRID_SIZE) * GRID_SIZE,
    y: Math.round(point.y / GRID_SIZE) * GRID_SIZE,
  };
}

/** 生成网格点的稳定映射键。 */
function pointKey(point: FlowPoint): string {
  return `${point.x},${point.y}`;
}

/** 曼哈顿距离是正交寻路的一致启发式函数。 */
function heuristic(a: FlowPoint, b: FlowPoint): number {
  return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
}

/** 根据父节点映射恢复从原点到目标的网格路径。 */
function reconstruct(parent: ReadonlyMap<string, string>, goalKey: string): FlowPoint[] {
  const points: FlowPoint[] = [];
  let current: string | undefined = goalKey;
  while (current) {
    const [x, y] = current.split(',').map(Number);
    points.push({ x, y });
    current = parent.get(current);
  }
  return points.reverse();
}

/** 移除重复点与共线中间点。 */
function simplifyPoints(points: ReadonlyArray<FlowPoint>): FlowPoint[] {
  const unique = points.filter((point, index) => (
    index === 0
    || point.x !== points[index - 1].x
    || point.y !== points[index - 1].y
  ));
  return unique.filter((point, index) => {
    if (index === 0 || index === unique.length - 1) return true;
    const previous = unique[index - 1];
    const next = unique[index + 1];
    return !(
      previous.x === point.x && point.x === next.x
      || previous.y === point.y && point.y === next.y
    );
  });
}

/** 计算正交折线的总曼哈顿长度。 */
function pathLength(points: ReadonlyArray<FlowPoint>): number {
  return points.slice(1).reduce((sum, point, index) => (
    sum + heuristic(points[index], point)
  ), 0);
}
