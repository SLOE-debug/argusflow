import type { FlowPoint, FlowRect } from "../types";
import { pointsBounds } from "./geometry";
import { RoutingObstacles } from "./obstacles";

/** 渲染和命中共享实际的直线及圆弧，不再对尖角折线命中。 */
export type PathSegment =
  | { readonly kind: "line"; readonly from: FlowPoint; readonly to: FlowPoint }
  | {
      readonly kind: "arc";
      readonly from: FlowPoint;
      readonly to: FlowPoint;
      readonly center: FlowPoint;
      readonly radius: number;
      readonly start: number;
      readonly sweep: number;
    };
export interface RoutedPath {
  readonly points: readonly FlowPoint[];
  readonly segments: readonly PathSegment[];
  readonly bounds: FlowRect;
  readonly status: "routed" | "blocked";
}
const TAU = Math.PI * 2;
const positive = (angle: number) => ((angle % TAU) + TAU) % TAU;
/** 弧区间始终不超过四分之一圆。 */
function onArc(
  angle: number,
  arc: Extract<PathSegment, { kind: "arc" }>,
): boolean {
  return (
    positive(arc.sweep > 0 ? angle - arc.start : arc.start - angle) <=
    Math.abs(arc.sweep) + 1e-8
  );
}
function arcCrosses(
  arc: Extract<PathSegment, { kind: "arc" }>,
  rect: FlowRect,
): boolean {
  const inside = (p: FlowPoint) =>
    p.x > rect.x + 1e-7 &&
    p.x < rect.x + rect.width - 1e-7 &&
    p.y > rect.y + 1e-7 &&
    p.y < rect.y + rect.height - 1e-7;
  if (inside(arc.from) || inside(arc.to)) return true;
  const { center, radius } = arc;
  // 两端与矩形边界相切时没有穿越交点，弧中段仍可能完全落在障碍内。
  const middle = arc.start + arc.sweep / 2;
  if (
    inside({
      x: center.x + Math.cos(middle) * radius,
      y: center.y + Math.sin(middle) * radius,
    })
  )
    return true;
  for (const x of [rect.x, rect.x + rect.width]) {
    const squared = radius * radius - (x - center.x) ** 2;
    if (squared <= 1e-8) continue;
    for (const y of [
      center.y - Math.sqrt(squared),
      center.y + Math.sqrt(squared),
    ])
      if (
        y > rect.y &&
        y < rect.y + rect.height &&
        onArc(Math.atan2(y - center.y, x - center.x), arc)
      )
        return true;
  }
  for (const y of [rect.y, rect.y + rect.height]) {
    const squared = radius * radius - (y - center.y) ** 2;
    if (squared <= 1e-8) continue;
    for (const x of [
      center.x - Math.sqrt(squared),
      center.x + Math.sqrt(squared),
    ])
      if (
        x > rect.x &&
        x < rect.x + rect.width &&
        onArc(Math.atan2(y - center.y, x - center.x), arc)
      )
        return true;
  }
  return false;
}
/** 去掉零长度与同向共线段，保留真正的反向折返。 */
export function simplifyPath(points: readonly FlowPoint[]): FlowPoint[] {
  const result: FlowPoint[] = [];
  for (const p of points) {
    const last = result.at(-1);
    if (last && Math.abs(last.x - p.x) + Math.abs(last.y - p.y) < 1e-8)
      continue;
    while (result.length >= 2) {
      const a = result.at(-2)!,
        b = result.at(-1)!;
      const cross = (b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x);
      const dot = (b.x - a.x) * (p.x - b.x) + (b.y - a.y) * (p.y - b.y);
      if (Math.abs(cross) > 1e-8 || dot <= 0) break;
      result.pop();
    }
    result.push(p);
  }
  return result;
}
/** 圆角缩小到不碰障碍为止；极窄通道保留精确折角。 */
export function roundedPath(
  input: readonly FlowPoint[],
  obstacles: RoutingObstacles,
  status: RoutedPath["status"],
): RoutedPath {
  const points = simplifyPath(input),
    segments: PathSegment[] = [];
  let from = points[0];
  for (let i = 1; i < points.length - 1; i++) {
    const a = points[i - 1],
      b = points[i],
      c = points[i + 1];
    const first = Math.hypot(b.x - a.x, b.y - a.y),
      second = Math.hypot(c.x - b.x, c.y - b.y);
    const before = { x: (b.x - a.x) / first, y: (b.y - a.y) / first },
      after = { x: (c.x - b.x) / second, y: (c.y - b.y) / second };
    const cross = before.x * after.y - before.y * after.x;
    let radius = Math.min(10, first / 2, second / 2);
    let arc: Extract<PathSegment, { kind: "arc" }> | null = null;
    while (Math.abs(cross) > 0.5 && radius >= 0.25) {
      const entry = { x: b.x - before.x * radius, y: b.y - before.y * radius },
        exit = { x: b.x + after.x * radius, y: b.y + after.y * radius };
      const center = {
        x: entry.x + after.x * radius,
        y: entry.y + after.y * radius,
      };
      const candidate: Extract<PathSegment, { kind: "arc" }> = {
        kind: "arc",
        from: entry,
        to: exit,
        center,
        radius,
        start: Math.atan2(entry.y - center.y, entry.x - center.x),
        sweep: (Math.sign(cross) * Math.PI) / 2,
      };
      if (
        !obstacles
          .query({
            x: center.x - radius,
            y: center.y - radius,
            width: radius * 2,
            height: radius * 2,
          })
          .some((rect) => arcCrosses(candidate, rect))
      ) {
        arc = candidate;
        break;
      }
      radius /= 2;
    }
    const to = arc?.from ?? b;
    if (Math.hypot(to.x - from.x, to.y - from.y) > 1e-8)
      segments.push({ kind: "line", from, to });
    if (arc) segments.push(arc);
    from = arc?.to ?? b;
  }
  const to = points.at(-1);
  if (from && to && Math.hypot(to.x - from.x, to.y - from.y) > 1e-8)
    segments.push({ kind: "line", from, to });
  return { points, segments, bounds: pointsBounds(points), status };
}
/** 精确圆弧距离保持所有缩放下的命中容差一致。 */
export function distanceToRoute(point: FlowPoint, path: RoutedPath): number {
  let distance = Infinity;
  for (const segment of path.segments) {
    const { from, to } = segment;
    let candidate: number;
    if (segment.kind === "arc") {
      const dx = point.x - segment.center.x,
        dy = point.y - segment.center.y;
      candidate = onArc(Math.atan2(dy, dx), segment)
        ? Math.abs(Math.hypot(dx, dy) - segment.radius)
        : Math.min(
            Math.hypot(point.x - from.x, point.y - from.y),
            Math.hypot(point.x - to.x, point.y - to.y),
          );
    } else {
      const dx = to.x - from.x,
        dy = to.y - from.y;
      const t = Math.max(
        0,
        Math.min(
          1,
          ((point.x - from.x) * dx + (point.y - from.y) * dy) /
            (dx * dx + dy * dy),
        ),
      );
      candidate = Math.hypot(
        point.x - from.x - t * dx,
        point.y - from.y - t * dy,
      );
    }
    distance = Math.min(distance, candidate);
  }
  return distance;
}
