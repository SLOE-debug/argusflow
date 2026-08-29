import {
  isRouteCoreClear,
  type RouteCollisionContext,
} from './routeCollision';
import { orthogonalConnectors } from './routeRepair';
import {
  orthogonalRoutePreferenceCost,
  simplifyOrthogonalPoints,
} from './routingGeometry';
import type { FlowPoint } from '../types';

/**
 * 把 OVG 或旧路修补产生的多折点主体反复压缩成更少折点的安全通道。
 *
 * 每轮从最长子路径开始尝试直连、L 形和 H/V 三段通道。候选只复用原路径
 * 已到达过的 x/y 坐标；只有完整通过碰撞检查、减少折点且整条路线的视觉
 * 偏好代价不增加时才替换，因此也能清掉复杂绕行路径中的局部微型楼梯。
 */
export function compactRouteCore(
  points: ReadonlyArray<FlowPoint>,
  collision: RouteCollisionContext,
): ReadonlyArray<FlowPoint> {
  let current: ReadonlyArray<FlowPoint> = simplifyOrthogonalPoints(points);
  let compacted: ReadonlyArray<FlowPoint> | null;
  do {
    compacted = findCompactedRoute(current, collision);
    if (compacted) current = compacted;
  } while (compacted);
  return current;
}

/** 从最长跨度开始寻找一个能降低整条路线复杂度的安全替换。 */
function findCompactedRoute(
  points: ReadonlyArray<FlowPoint>,
  collision: RouteCollisionContext,
): ReadonlyArray<FlowPoint> | null {
  const currentCost = orthogonalRoutePreferenceCost(points);
  for (let spanLength = points.length; spanLength >= 3; spanLength -= 1) {
    for (let startIndex = 0; startIndex + spanLength <= points.length; startIndex += 1) {
      const endIndex = startIndex + spanLength - 1;
      const span = points.slice(startIndex, endIndex + 1);
      /** 坐标通道来自当前子路径，可覆盖其外围与局部绕行基线。 */
      const channelXCoordinates = new Set(span.map((point) => point.x));
      const channelYCoordinates = new Set(span.map((point) => point.y));
      const candidates = createChannelCandidates(
        span[0],
        span.at(-1)!,
        channelXCoordinates,
        channelYCoordinates,
      );
      for (const candidate of candidates) {
        const simplified = simplifyOrthogonalPoints(candidate);
        if (
          simplified.length >= span.length
          || !isRouteCoreClear(simplified, collision)
        ) continue;
        const next = simplifyOrthogonalPoints([
          ...points.slice(0, startIndex),
          ...simplified,
          ...points.slice(endIndex + 1),
        ]);
        if (
          next.length >= points.length
          || orthogonalRoutePreferenceCost(next) > currentCost
        ) continue;
        return next;
      }
    }
  }
  return null;
}

/** 枚举直连、L 形以及沿既有 x/y 基线的 H/V 三段通道。 */
function createChannelCandidates(
  start: FlowPoint,
  end: FlowPoint,
  xCoordinates: ReadonlySet<number>,
  yCoordinates: ReadonlySet<number>,
): ReadonlyArray<ReadonlyArray<FlowPoint>> {
  const candidates: ReadonlyArray<FlowPoint>[] = [
    ...orthogonalConnectors(start, end),
  ];
  for (const channelX of xCoordinates) {
    candidates.push([
      start,
      { x: channelX, y: start.y },
      { x: channelX, y: end.y },
      end,
    ]);
  }
  for (const channelY of yCoordinates) {
    candidates.push([
      start,
      { x: start.x, y: channelY },
      { x: end.x, y: channelY },
      end,
    ]);
  }
  return candidates;
}
