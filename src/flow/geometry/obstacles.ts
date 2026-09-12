import type { FlowPoint, FlowRect } from "../types";

/** 障碍物带稳定身份，只有自身端口的出入引线可穿过其外扩区。 */
export interface FlowObstacle extends FlowRect {
  readonly id: string;
}
export const ROUTE_CLEARANCE = 12;
/** 正交线段是否进入矩形内部；允许沿外扩边界行走。 */
export function segmentCrosses(
  a: FlowPoint,
  b: FlowPoint,
  rect: FlowRect,
): boolean {
  if (a.x === b.x)
    return (
      a.x > rect.x + 1e-7 &&
      a.x < rect.x + rect.width - 1e-7 &&
      Math.max(a.y, b.y) > rect.y + 1e-7 &&
      Math.min(a.y, b.y) < rect.y + rect.height - 1e-7
    );
  if (a.y === b.y)
    return (
      a.y > rect.y + 1e-7 &&
      a.y < rect.y + rect.height - 1e-7 &&
      Math.max(a.x, b.x) > rect.x + 1e-7 &&
      Math.min(a.x, b.x) < rect.x + rect.width - 1e-7
    );
  return true;
}
/** 按左边界排序并缓存前缀右边界，线段查询跳过无关障碍。 */
export class RoutingObstacles {
  readonly rects: readonly FlowObstacle[];
  private readonly rights: readonly number[];
  constructor(obstacles: readonly FlowObstacle[]) {
    this.rects = obstacles
      .map((rect) => ({
        ...rect,
        x: rect.x - ROUTE_CLEARANCE,
        y: rect.y - ROUTE_CLEARANCE,
        width: rect.width + ROUTE_CLEARANCE * 2,
        height: rect.height + ROUTE_CLEARANCE * 2,
      }))
      .sort((a, b) => a.x - b.x || a.id.localeCompare(b.id));
    let right = -Infinity;
    this.rights = this.rects.map(
      (rect) => (right = Math.max(right, rect.x + rect.width)),
    );
  }
  /** 包围盒查询供直线及精确圆弧碰撞共同使用。 */
  query(bounds: FlowRect): readonly FlowObstacle[] {
    let low = 0,
      high = this.rects.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (this.rights[mid] < bounds.x) low = mid + 1;
      else high = mid;
    }
    const result: FlowObstacle[] = [];
    for (let i = low; i < this.rects.length; i++) {
      const rect = this.rects[i];
      if (rect.x > bounds.x + bounds.width) break;
      if (
        rect.x + rect.width >= bounds.x &&
        rect.y <= bounds.y + bounds.height &&
        rect.y + rect.height >= bounds.y
      )
        result.push(rect);
    }
    return result;
  }
  clear(a: FlowPoint, b: FlowPoint, owner?: string): boolean {
    const bounds = {
      x: Math.min(a.x, b.x),
      y: Math.min(a.y, b.y),
      width: Math.abs(a.x - b.x),
      height: Math.abs(a.y - b.y),
    };
    return !this.query(bounds).some(
      (rect) => rect.id !== owner && segmentCrosses(a, b, rect),
    );
  }
}
