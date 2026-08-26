import { pointsBounds, rectsIntersect } from './geometry';
import type { FlowPoint, FlowRect, RoutedEdge } from './types';

/** 删除重复点与共线中间点，保持正交折线的最小表达。 */
export function simplifyOrthogonalPoints(
  points: ReadonlyArray<FlowPoint>,
): FlowPoint[] {
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

/** 把正交线段转换为允许零厚度的精确包围盒。 */
export function segmentBounds(a: FlowPoint, b: FlowPoint): FlowRect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

/** 计算折线的曼哈顿总长度。 */
export function orthogonalPathLength(points: ReadonlyArray<FlowPoint>): number {
  return points.slice(1).reduce((sum, point, index) => (
    sum + manhattanDistance(points[index], point)
  ), 0);
}

/** 计算两点的曼哈顿距离。 */
export function manhattanDistance(a: FlowPoint, b: FlowPoint): number {
  return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
}

/** 合并两个矩形的最小包围盒。 */
export function unionRects(a: FlowRect, b: FlowRect): FlowRect {
  const left = Math.min(a.x, b.x);
  const top = Math.min(a.y, b.y);
  const right = Math.max(a.x + a.width, b.x + b.width);
  const bottom = Math.max(a.y + a.height, b.y + b.height);
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** 向四周扩展矩形，用作局部寻路走廊。 */
export function inflateRect(rect: FlowRect, amount: number): FlowRect {
  return {
    x: rect.x - amount,
    y: rect.y - amount,
    width: rect.width + amount * 2,
    height: rect.height + amount * 2,
  };
}

/** 判断正交线段是否与矩形闭边界相交。 */
export function segmentIntersectsRect(
  a: FlowPoint,
  b: FlowPoint,
  rect: FlowRect,
): boolean {
  return rectsIntersect(segmentBounds(a, b), rect);
}

/** 把折点构造成渲染所需的完整路由对象。 */
export function createRoutedEdge(
  edgeId: string,
  points: ReadonlyArray<FlowPoint>,
  sourceSide: RoutedEdge['sourceSide'],
  targetSide: RoutedEdge['targetSide'],
  preserveEndpointSegments = false,
): RoutedEdge {
  const simplifiedPoints = preserveEndpointSegments
    ? simplifyRouteWithEndpointSegments(points)
    : simplifyOrthogonalPoints(points);
  return {
    edgeId,
    points: simplifiedPoints,
    sourceSide,
    targetSide,
    path: roundedOrthogonalPath(simplifiedPoints),
    bounds: pointsBounds(simplifiedPoints),
  };
}

/** 简化主体折点，同时保留 source escape 与 target escape 两个端口约束点。 */
function simplifyRouteWithEndpointSegments(
  points: ReadonlyArray<FlowPoint>,
): FlowPoint[] {
  const unique = points.filter((point, index) => (
    index === 0
    || point.x !== points[index - 1].x
    || point.y !== points[index - 1].y
  ));
  return unique.filter((point, index) => {
    if (
      index <= 1
      || index >= unique.length - 2
    ) return true;
    const previous = unique[index - 1];
    const next = unique[index + 1];
    return !(
      previous.x === point.x && point.x === next.x
      || previous.y === point.y && point.y === next.y
    );
  });
}

/** 把正交折线转换为包含二次曲线圆角的 SVG path。 */
export function roundedOrthogonalPath(
  points: ReadonlyArray<FlowPoint>,
  radius = 12,
): string {
  if (points.length === 0) return '';
  if (points.length === 1) return `M ${points[0].x} ${points[0].y}`;
  let path = `M ${points[0].x} ${points[0].y}`;
  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1];
    const current = points[index];
    const next = points[index + 1];
    const incoming = Math.min(radius, euclideanDistance(previous, current) / 2);
    const outgoing = Math.min(radius, euclideanDistance(current, next) / 2);
    const before = moveToward(current, previous, incoming);
    const after = moveToward(current, next, outgoing);
    path += ` L ${before.x} ${before.y} Q ${current.x} ${current.y} ${after.x} ${after.y}`;
  }
  const last = points.at(-1)!;
  return `${path} L ${last.x} ${last.y}`;
}

/** 判断矩形是否具有相同位置与尺寸。 */
export function rectEquals(a: FlowRect, b: FlowRect): boolean {
  return a.x === b.x
    && a.y === b.y
    && a.width === b.width
    && a.height === b.height;
}

/** 欧氏距离仅用于计算圆角截断长度。 */
function euclideanDistance(a: FlowPoint, b: FlowPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/** 沿两点连线移动指定距离。 */
function moveToward(from: FlowPoint, to: FlowPoint, amount: number): FlowPoint {
  const length = euclideanDistance(from, to) || 1;
  return {
    x: from.x + (to.x - from.x) / length * amount,
    y: from.y + (to.y - from.y) / length * amount,
  };
}
