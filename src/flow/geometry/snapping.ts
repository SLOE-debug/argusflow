import type { FlowPoint, FlowRect } from "../types";

/** 吸附与脱离共用 CSS 像素范围，不随画布缩放变得更粘。 */
export const SNAP_DISTANCE = 5;
/** 浮点运算误差不应改变等距参考线的优先级或边界命中。 */
const EPSILON = 1e-9;
/** 局部坐标中的对齐线；x 表示竖线，y 表示横线。 */
export interface SnapGuide {
  readonly axis: "x" | "y";
  readonly position: number;
  readonly start: number;
  readonly end: number;
}
/** 吸附只修正本次位移，参考线不属于文档。 */
export interface SnapResult {
  readonly delta: FlowPoint;
  readonly guides: readonly SnapGuide[];
}
/** 两侧边缘仅匹配同侧边缘，中心仅匹配中心，避免靠近时贴到卡片内部。 */
type Alignment = "start" | "center" | "end";
interface Reference {
  readonly alignment: Alignment;
  readonly position: number;
  readonly start: number;
  readonly end: number;
}
interface Match {
  readonly reference: Reference;
  readonly correction: number;
  readonly gap: number;
}

function references(rect: FlowRect, axis: "x" | "y"): readonly Reference[] {
  const size = axis === "x" ? rect.width : rect.height;
  const start = axis === "x" ? rect.y : rect.x;
  const end = start + (axis === "x" ? rect.height : rect.width);
  return [
    { alignment: "start", position: rect[axis], start, end },
    { alignment: "center", position: rect[axis] + size / 2, start, end },
    { alignment: "end", position: rect[axis] + size, start, end },
  ];
}

function lowerBound(lines: readonly Reference[], position: number): number {
  let low = 0,
    high = lines.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (lines[middle].position < position) low = middle + 1;
    else high = middle;
  }
  return low;
}

/** 等距离时选附近对象，再优先中心；结果不依赖上一次指针事件。 */
function compare(a: Match, b: Match): number {
  const distance = Math.abs(a.correction) - Math.abs(b.correction);
  return (
    (Math.abs(distance) > EPSILON ? distance : 0) ||
    a.gap - b.gap ||
    Number(a.reference.alignment !== "center") -
      Number(b.reference.alignment !== "center") ||
    a.reference.position - b.reference.position ||
    a.reference.start - b.reference.start ||
    a.reference.end - b.reference.end
  );
}

function closest(
  lines: readonly Reference[],
  moving: readonly Reference[],
  tolerance: number,
): Match | null {
  let best: Match | null = null;
  for (const anchor of moving) {
    const first = lowerBound(lines, anchor.position - tolerance);
    for (let index = first; index < lines.length; index++) {
      const reference = lines[index];
      if (reference.position > anchor.position + tolerance) break;
      if (reference.alignment !== anchor.alignment) continue;
      const candidate: Match = {
        reference,
        correction: reference.position - anchor.position,
        gap: Math.max(
          0,
          reference.start - anchor.end,
          anchor.start - reference.end,
        ),
      };
      if (!best || compare(candidate, best) < 0) best = candidate;
    }
  }
  return best;
}

/** 拖动开始时建立固定对象索引；移动时只查询容差内的参考位置。 */
export class SnapIndex {
  private readonly x: readonly Reference[];
  private readonly y: readonly Reference[];

  constructor(rects: readonly FlowRect[]) {
    const index = (axis: "x" | "y") =>
      rects
        .flatMap((rect) => references(rect, axis))
        .sort((a, b) => a.position - b.position);
    this.x = index("x");
    this.y = index("y");
  }

  /** delta 必须来自按下时的原始鼠标位移，禁止累加吸附后的位移。 */
  snap(bounds: FlowRect, delta: FlowPoint, zoom: number): SnapResult {
    if ((!delta.x && !delta.y) || !Number.isFinite(zoom) || zoom <= 0)
      return { delta, guides: [] };
    const placed = { ...bounds, x: bounds.x + delta.x, y: bounds.y + delta.y };
    const tolerance = SNAP_DISTANCE / zoom + EPSILON;
    const x = closest(this.x, references(placed, "x"), tolerance);
    const y = closest(this.y, references(placed, "y"), tolerance);
    /** 直接从原始边界计算精确落点，避免原位移加修正留下浮点尾差。 */
    const displacement = (axis: "x" | "y", match: Match | null) => {
      if (!match) return delta[axis];
      const origin = references(bounds, axis).find(
        (anchor) => anchor.alignment === match.reference.alignment,
      )!;
      return match.reference.position - origin.position;
    };
    const snapped = { x: displacement("x", x), y: displacement("y", y) };
    const guides: SnapGuide[] = [];
    for (const [axis, match] of [
      ["x", x],
      ["y", y],
    ] as const) {
      if (!match) continue;
      /** 线段使用两轴修正后的边界，避免斜向拖动时参考线端点滞后。 */
      const start = axis === "x" ? bounds.y + snapped.y : bounds.x + snapped.x;
      const end = start + (axis === "x" ? bounds.height : bounds.width);
      guides.push({
        axis,
        position: match.reference.position,
        start: Math.min(start, match.reference.start),
        end: Math.max(end, match.reference.end),
      });
    }
    return { delta: snapped, guides };
  }
}
