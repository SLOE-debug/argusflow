import { rectsIntersect } from './geometry';
import type { ObstacleIndex } from './obstacleIndex';
import { segmentBounds, segmentIntersectsRect } from './routingGeometry';
import type { FlowPoint, FlowRect } from './types';

/** 单条边主体碰撞检测所需的稳定上下文。 */
export type RouteCollisionContext = Readonly<{
  obstacles: ObstacleIndex;
  excludedNodeIds: ReadonlySet<string>;
  endpointRects: ReadonlyArray<FlowRect>;
}>;

/** 判断一条正交折线是否避开普通障碍物及源目标真实节点体。 */
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
  const crossesObstacle = context.obstacles.query(bounds).some((obstacle) => (
    !context.excludedNodeIds.has(obstacle.nodeId)
    && segmentIntersectsRect(start, end, obstacle.rect)
  ));
  if (crossesObstacle) return false;
  return !context.endpointRects.some((rect) => (
    segmentIntersectsRect(start, end, rect)
  ));
}

/** 判断强制端口直线段是否避开源目标之外的节点安全区。 */
export function isRoutingPortTunnelClear(
  anchor: FlowPoint,
  escape: FlowPoint,
  context: RouteCollisionContext,
): boolean {
  const bounds = segmentBounds(anchor, escape);
  return !context.obstacles.query(bounds).some((obstacle) => (
    !context.excludedNodeIds.has(obstacle.nodeId)
    && segmentIntersectsRect(anchor, escape, obstacle.rect)
  ));
}

/** 判断一个候选点是否落入任一禁止区域。 */
export function isRoutePointBlocked(
  point: FlowPoint,
  context: RouteCollisionContext,
): boolean {
  const pointRect = { x: point.x, y: point.y, width: 0, height: 0 };
  const insideObstacle = context.obstacles.query(pointRect).some((obstacle) => (
    !context.excludedNodeIds.has(obstacle.nodeId)
    && rectsIntersect(pointRect, obstacle.rect)
  ));
  return insideObstacle || context.endpointRects.some((rect) => (
    rectsIntersect(pointRect, rect)
  ));
}
