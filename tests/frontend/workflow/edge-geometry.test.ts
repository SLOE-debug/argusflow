import { expect, it } from "vitest";
import {
  rectAnchor,
  routeEdge,
  RoutingObstacles,
  distanceToRoute,
  type RoutedPath,
  type FlowRect,
} from "../../../src/flow";
import {
  ANCHOR_NORMALS,
  ANCHOR_SIDES,
} from "../../../src/flow/geometry/anchors";
import { roundedPath } from "../../../src/flow/geometry/rounded-path";

function samples(path: RoutedPath) {
  return path.segments.flatMap((segment) =>
    Array.from({ length: 101 }, (_, i) => {
      const t = i / 100;
      return segment.kind === "line"
        ? {
            x: segment.from.x + (segment.to.x - segment.from.x) * t,
            y: segment.from.y + (segment.to.y - segment.from.y) * t,
          }
        : {
            x:
              segment.center.x +
              Math.cos(segment.start + segment.sweep * t) * segment.radius,
            y:
              segment.center.y +
              Math.sin(segment.start + segment.sweep * t) * segment.radius,
          };
    }),
  );
}
function outside(path: RoutedPath, rects: readonly FlowRect[]) {
  for (const p of samples(path))
    expect(
      rects.some(
        (r) =>
          p.x > r.x + 1e-6 &&
          p.x < r.x + r.width - 1e-6 &&
          p.y > r.y + 1e-6 &&
          p.y < r.y + r.height - 1e-6,
      ),
    ).toBe(false);
}
it("圆弧两端仅与障碍相切时也检查弧内部", () => {
  const obstacles = new RoutingObstacles([
    { id: "corner", x: 22, y: 12, width: 100, height: 100 },
  ]);
  const path = roundedPath(
    [
      { x: 40, y: 0 },
      { x: 10, y: 0 },
      { x: 10, y: 30 },
    ],
    obstacles,
    "routed",
  );
  outside(path, obstacles.rects);
  expect(path.segments.every((segment) => segment.kind === "line")).toBe(true);
});
const source = { id: "source", x: 0, y: 0, width: 100, height: 80 };
const target = { id: "target", x: 400, y: 220, width: 100, height: 80 };
it.each(
  ANCHOR_SIDES.flatMap((from) => ANCHOR_SIDES.map((to) => [from, to] as const)),
)("%s → %s 遵守法线并避开端点卡片", (from, to) => {
  const a = rectAnchor(source, from, source.id),
    b = rectAnchor(target, to, target.id);
  const obstacles = new RoutingObstacles([source, target]);
  const path = routeEdge(a, b, obstacles);
  expect(path.status).toBe("routed");
  expect(path.points[0]).toEqual(a.point);
  expect(path.points.at(-1)).toEqual(b.point);
  const first = path.segments[0],
    last = path.segments.at(-1)!;
  expect(first.kind).toBe("line");
  expect(last.kind).toBe("line");
  expect(
    (first.to.x - first.from.x) * ANCHOR_NORMALS[from].x +
      (first.to.y - first.from.y) * ANCHOR_NORMALS[from].y,
  ).toBeGreaterThan(0);
  expect(
    (last.to.x - last.from.x) * ANCHOR_NORMALS[to].x +
      (last.to.y - last.from.y) * ANCHOR_NORMALS[to].y,
  ).toBeLessThan(0);
  outside(path, [source, target]);
  expect(routeEdge(a, b, obstacles)).toEqual(path);
});
it("反向连线与多障碍保留圆角和 12 单位间距", () => {
  const obstacles = new RoutingObstacles([
    { id: "a", x: 130, y: -80, width: 90, height: 200 },
    { id: "b", x: 280, y: 20, width: 80, height: 180 },
  ]);
  const path = routeEdge(
    { point: { x: 500, y: 60 }, side: "right" },
    { point: { x: 0, y: 60 }, side: "left" },
    obstacles,
  );
  expect(path.status).toBe("routed");
  expect(path.segments.filter((s) => s.kind === "arc").length).toBeGreaterThan(
    1,
  );
  outside(path, obstacles.rects);
});
it("窄通道可直行，相邻节点缩短引线，重叠布局明确受阻", () => {
  const obstacles = new RoutingObstacles([
    { id: "top", x: 100, y: -100, width: 200, height: 86 },
    { id: "bottom", x: 100, y: 14, width: 200, height: 86 },
  ]);
  const path = routeEdge(
    { point: { x: 0, y: 0 }, side: "right" },
    { point: { x: 400, y: 0 }, side: "left" },
    obstacles,
  );
  expect(path.status).toBe("routed");
  outside(path, obstacles.rects);
  const near = { ...target, x: 130, y: 0 };
  expect(
    routeEdge(
      rectAnchor(source, "right", source.id),
      rectAnchor(near, "left", near.id),
      new RoutingObstacles([source, near]),
    ).status,
  ).toBe("routed");
  const overlap = { ...target, x: 80, y: 0 };
  expect(
    routeEdge(
      rectAnchor(source, "right", source.id),
      rectAnchor(overlap, "left", overlap.id),
      new RoutingObstacles([source, overlap]),
    ).status,
  ).toBe("blocked");
});
it("命中使用实际圆弧距离，短线段缩小圆角，拐角切入障碍时缩小至安全", () => {
  const path = roundedPath(
    [
      { x: 0, y: 0 },
      { x: 20, y: 0 },
      { x: 20, y: 20 },
    ],
    new RoutingObstacles([]),
    "routed",
  );
  const arc = path.segments.find((s) => s.kind === "arc")!;
  if (arc.kind !== "arc") throw new Error("missing arc");
  const angle = arc.start + arc.sweep / 2;
  expect(
    distanceToRoute(
      {
        x: arc.center.x + Math.cos(angle) * 10,
        y: arc.center.y + Math.sin(angle) * 10,
      },
      path,
    ),
  ).toBeCloseTo(0);
  expect(distanceToRoute({ x: 20, y: 0 }, path)).toBeCloseTo(
    Math.sqrt(200) - 10,
  );
  const short = roundedPath(
    [
      { x: 0, y: 0 },
      { x: 4, y: 0 },
      { x: 4, y: 4 },
    ],
    new RoutingObstacles([]),
    "routed",
  );
  expect(short.segments.find((s) => s.kind === "arc")).toMatchObject({
    radius: 2,
  });
  const obstacle = new RoutingObstacles([
    { id: "corner", x: -20, y: 13, width: 27, height: 30 },
  ]);
  const safe = roundedPath(
    [
      { x: 0, y: 0 },
      { x: 20, y: 0 },
      { x: 20, y: 50 },
    ],
    obstacle,
    "routed",
  );
  outside(safe, obstacle.rects);
});
