import type { FlowPoint, FlowRect, ViewportTransform } from "../types";
import { transformPoint } from "./scene";

/** 将局部矩形投影到父坐标系；缩放必须为正数。 */
export function transformRect(
  rect: FlowRect,
  view: ViewportTransform,
): FlowRect {
  return {
    ...transformPoint(rect, view),
    width: rect.width * view.zoom,
    height: rect.height * view.zoom,
  };
}

/** 点到折线的最短距离。调用方将屏幕命中容差换算到同一坐标系。 */
export function distanceToPath(
  point: FlowPoint,
  path: readonly FlowPoint[],
): number {
  let distance = Infinity;
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1],
      b = path[i];
    const dx = b.x - a.x,
      dy = b.y - a.y;
    const length = dx * dx + dy * dy;
    const t = length
      ? Math.max(
          0,
          Math.min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / length),
        )
      : 0;
    distance = Math.min(
      distance,
      Math.hypot(point.x - a.x - t * dx, point.y - a.y - t * dy),
    );
  }
  return distance;
}
