import type { FlowPoint } from "../types";
import { ANCHOR_NORMALS, type FlowAnchor } from "./anchors";
import { RoutingObstacles, ROUTE_CLEARANCE, segmentCrosses } from "./obstacles";
import { roundedPath, simplifyPath, type RoutedPath } from "./rounded-path";
import { searchRoute } from "./routing-search";

/** 同长度下优先少转弯，避免节点亚像素移动时在等价路线间抖动。 */
export function routeCost(points: readonly FlowPoint[]): number {
  const path = simplifyPath(points);
  return (
    path.reduce(
      (sum, point, i) =>
        i
          ? sum +
            Math.abs(point.x - path[i - 1].x) +
            Math.abs(point.y - path[i - 1].y)
          : sum,
      0,
    ) +
    Math.max(0, path.length - 2) * 24
  );
}
/** 四边锚点使用相同的障碍路由，节点拖动与自由端预览也调用此入口。 */
export function routeEdge(
  source: FlowAnchor,
  target: FlowAnchor,
  obstacles: RoutingObstacles,
): RoutedPath {
  const sourceNormal = ANCHOR_NORMALS[source.side];
  const targetNormal = ANCHOR_NORMALS[target.side];
  const dx = target.point.x - source.point.x;
  const dy = target.point.y - source.point.y;
  /** 相向端口按轴向净间距共享引线空间，避免两条 24 单位引线交错后绕圈。 */
  const facing =
    sourceNormal.x === -targetNormal.x && sourceNormal.y === -targetNormal.y;
  const gap = dx * sourceNormal.x + dy * sourceNormal.y;
  if (facing && gap > 0 && dx * sourceNormal.y - dy * sourceNormal.x === 0) {
    const bounds = {
      x: Math.min(source.point.x, target.point.x),
      y: Math.min(source.point.y, target.point.y),
      width: Math.abs(dx),
      height: Math.abs(dy),
    };
    /** 短直连允许经过两端自己的外扩区，其他障碍仍必须避开。 */
    const clear = obstacles
      .query(bounds)
      .every(
        (rect) =>
          rect.id === source.owner ||
          rect.id === target.owner ||
          !segmentCrosses(source.point, target.point, rect),
      );
    if (clear)
      return roundedPath([source.point, target.point], obstacles, "routed");
  }
  /** 转弯引线必须先离开自身外扩区；轴向不足时保留绕行所需的空间。 */
  const maximumLead =
    facing && gap >= ROUTE_CLEARANCE * 2 ? Math.min(24, gap / 2) : 24;
  const lead = (anchor: FlowAnchor): FlowPoint => {
    const normal = ANCHOR_NORMALS[anchor.side];
    const candidates = [
      maximumLead,
      ...[18, 12].filter((distance) => distance < maximumLead),
    ].map((distance) => ({
      x: anchor.point.x + normal.x * distance,
      y: anchor.point.y + normal.y * distance,
    }));
    return (
      candidates.find((point) =>
        obstacles.clear(anchor.point, point, anchor.owner),
      ) ?? candidates[0]
    );
  };
  const start = lead(source),
    end = lead(target);
  const middleX = (start.x + end.x) / 2,
    middleY = (start.y + end.y) / 2;
  const candidates: FlowPoint[][] = [
    [start, { x: end.x, y: start.y }, end],
    [start, { x: start.x, y: end.y }, end],
    [start, { x: middleX, y: start.y }, { x: middleX, y: end.y }, end],
    [start, { x: start.x, y: middleY }, { x: end.x, y: middleY }, end],
  ];
  const wrap = (path: readonly FlowPoint[]) => [
    source.point,
    ...path,
    target.point,
  ];
  const leadsClear =
    obstacles.clear(source.point, start, source.owner) &&
    obstacles.clear(end, target.point, target.owner);
  const valid = leadsClear
    ? candidates.filter((path) => {
        const full = simplifyPath(wrap(path));
        return (
          full.every(
            (p, i) =>
              i < 2 ||
              (p.x - full[i - 1].x) * (full[i - 1].x - full[i - 2].x) +
                (p.y - full[i - 1].y) * (full[i - 1].y - full[i - 2].y) >=
                0,
          ) && path.every((p, i) => !i || obstacles.clear(path[i - 1], p))
        );
      })
    : [];
  valid.sort((a, b) => routeCost(wrap(a)) - routeCost(wrap(b)));
  const path =
    valid[0] ??
    (leadsClear
      ? searchRoute(
          start,
          end,
          obstacles,
          ANCHOR_NORMALS[source.side],
          ANCHOR_NORMALS[target.side],
        )
      : null);
  return roundedPath(
    wrap(path ?? candidates[0]),
    obstacles,
    path ? "routed" : "blocked",
  );
}
