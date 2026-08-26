import { rectsIntersect } from './geometry';
import type { ObstacleIndex } from './obstacleIndex';
import { segmentBounds, segmentIntersectsRect } from './routingGeometry';
import type { FlowPoint, FlowRect } from './types';
import type { RoutingPort } from './routingTypes';

/** 单条边主体碰撞检测所需的稳定上下文。 */
export type RouteCollisionContext = Readonly<{
  /** 包含普通节点与两个端点膨胀安全区的长期空间索引。 */
  obstacles: ObstacleIndex;
  /** 两个端点节点 ID；仅用于可见图避免重复加入同一障碍物。 */
  endpointNodeIds: ReadonlySet<string>;
  /** 两个端点的膨胀安全区，供局部可见图生成绕行 portal。 */
  endpointKeepOutRects: ReadonlyArray<FlowRect>;
}>;

/** 判断一条正交折线是否避开包括源、目标在内的全部节点安全区。 */
export function isRouteCoreClear(
  points: ReadonlyArray<FlowPoint>,
  context: RouteCollisionContext,
): boolean {
  for (let index = 1; index < points.length; index += 1) {
    if (!isRouteSegmentClear(points[index - 1], points[index], context)) {
      return false;
    }
  }
  return true;
}

/** 判断单条水平或垂直线段是否可通行。 */
export function isRouteSegmentClear(
  start: FlowPoint,
  end: FlowPoint,
  context: RouteCollisionContext,
): boolean {
  if (start.x !== end.x && start.y !== end.y) return false;
  const bounds = segmentBounds(start, end);
  return !context.obstacles.query(bounds).some((obstacle) => (
    segmentIntersectsRect(start, end, obstacle.rect)
  ));
}

/**
 * 判断强制端口直线段是否避开所属端点之外的全部节点安全区。
 *
 * 只有 port.nodeId 对应的安全区可被 tunnel 穿过；另一端点仍是普通障碍物。
 */
export function isRoutingPortTunnelClear(
  port: RoutingPort,
  context: RouteCollisionContext,
): boolean {
  const bounds = segmentBounds(port.anchor, port.escape);
  return !context.obstacles.query(bounds).some((obstacle) => (
    obstacle.nodeId !== port.nodeId
    && segmentIntersectsRect(port.anchor, port.escape, obstacle.rect)
  ));
}

/** 判断一个候选点是否落入任一禁止区域。 */
export function isRoutePointBlocked(
  point: FlowPoint,
  context: RouteCollisionContext,
): boolean {
  const pointRect = { x: point.x, y: point.y, width: 0, height: 0 };
  return context.obstacles.query(pointRect).some((obstacle) => (
    rectsIntersect(pointRect, obstacle.rect)
  ));
}
