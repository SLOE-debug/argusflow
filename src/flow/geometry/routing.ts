import type { FlowPoint, FlowRect } from "../types";
/** 同域正交路径绕开无关节点；重叠布局仍保留可编辑的连线。 */
export function routeEdge(
  source: FlowPoint,
  target: FlowPoint,
  obstacles: readonly FlowRect[],
): readonly FlowPoint[] {
  const start = { x: source.x + 20, y: source.y },
    end = { x: target.x - 20, y: target.y };
  const crosses = (a: FlowPoint, b: FlowPoint) =>
    obstacles.some((rect) =>
      a.x === b.x
        ? a.x > rect.x - 8 &&
          a.x < rect.x + rect.width + 8 &&
          Math.max(a.y, b.y) > rect.y - 8 &&
          Math.min(a.y, b.y) < rect.y + rect.height + 8
        : a.y > rect.y - 8 &&
          a.y < rect.y + rect.height + 8 &&
          Math.max(a.x, b.x) > rect.x - 8 &&
          Math.min(a.x, b.x) < rect.x + rect.width + 8,
    );
  const middle = (start.x + end.x) / 2;
  const candidates: FlowPoint[][] = [
    [start, { x: middle, y: start.y }, { x: middle, y: end.y }, end],
  ];
  const lanes = new Set([
    source.y,
    target.y,
    ...obstacles.flatMap((rect) => [rect.y - 24, rect.y + rect.height + 24]),
  ]);
  for (const y of lanes)
    candidates.push([start, { x: start.x, y }, { x: end.x, y }, end]);
  const valid = candidates.filter((path) =>
    path.every((point, index) => !index || !crosses(path[index - 1], point)),
  );
  const distance = (path: readonly FlowPoint[]) =>
    path.reduce(
      (sum, point, index) =>
        index
          ? sum +
            Math.abs(point.x - path[index - 1].x) +
            Math.abs(point.y - path[index - 1].y)
          : sum,
      0,
    );
  valid.sort((a, b) => distance(a) - distance(b));
  const path = [source, ...(valid[0] ?? candidates[0]), target];
  return path.filter(
    (point, index) =>
      !index || point.x !== path[index - 1].x || point.y !== path[index - 1].y,
  );
}
export function routePath(points: readonly FlowPoint[]): string {
  return points
    .map((point, index) => (index ? "L " : "M ") + point.x + " " + point.y)
    .join(" ");
}
