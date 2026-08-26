import {
  isRouteCoreClear,
  type RouteCollisionContext,
} from './routeCollision';
import {
  orthogonalPathLength,
  simplifyOrthogonalPoints,
} from './routingGeometry';
import type { FlowPoint, RoutedEdge } from './types';
import type { RoutingPort } from './routingTypes';

/** 旧路径一侧最多尝试保留的内部折点数量，限制热路径组合规模。 */
const MAX_REPAIR_POINTS_PER_SIDE = 4;

/**
 * 把新端口接回上一帧路径的内部折点，仅检查少量新连接与复用主体。
 *
 * 成功时返回 source.escape 到 target.escape 的主体；失败才进入局部 OVG。
 */
export function repairRouteCore(
  previous: RoutedEdge,
  source: RoutingPort,
  target: RoutingPort,
  collision: RouteCollisionContext,
): ReadonlyArray<FlowPoint> | null {
  const previousCore = previous.points.slice(1, -1);
  if (previousCore.length === 0) return null;
  const sourceLimit = Math.min(
    previousCore.length - 1,
    MAX_REPAIR_POINTS_PER_SIDE,
  );
  /** 最短可行修补会在候选评分中胜出，避免首个命中产生多余折点。 */
  let best: ReadonlyArray<FlowPoint> | null = null;
  let bestLength = Number.POSITIVE_INFINITY;

  for (let sourceIndex = 0; sourceIndex <= sourceLimit; sourceIndex += 1) {
    const minimumTargetIndex = Math.max(sourceIndex, previousCore.length - 1
      - MAX_REPAIR_POINTS_PER_SIDE);
    for (
      let targetIndex = previousCore.length - 1;
      targetIndex >= minimumTargetIndex;
      targetIndex -= 1
    ) {
      const retained = previousCore.slice(sourceIndex, targetIndex + 1);
      for (const sourceConnector of orthogonalConnectors(
        source.escape,
        retained[0],
      )) {
        for (const targetConnector of orthogonalConnectors(
          retained.at(-1)!,
          target.escape,
        )) {
          const candidate = simplifyOrthogonalPoints([
            ...sourceConnector,
            ...retained.slice(1),
            ...targetConnector.slice(1),
          ]);
          if (!isRouteCoreClear(candidate, collision)) continue;
          const length = orthogonalPathLength(candidate);
          if (length >= bestLength) continue;
          best = candidate;
          bestLength = length;
        }
      }
    }
  }
  return best;
}

/** 生成直连和两种 L 形接入，调用方统一执行碰撞检查。 */
export function orthogonalConnectors(
  start: FlowPoint,
  end: FlowPoint,
): ReadonlyArray<ReadonlyArray<FlowPoint>> {
  if (start.x === end.x || start.y === end.y) return [[start, end]];
  return [
    [start, { x: end.x, y: start.y }, end],
    [start, { x: start.x, y: end.y }, end],
  ];
}
