import { expect, it } from "vitest";
import {
  ANCHOR_NORMALS,
  rectAnchor,
  routeEdge,
  RoutingObstacles,
  type FlowAnchorSide,
} from "../../../src/flow";

const source = { id: "source", x: 0, y: 0, width: 232, height: 80 };
const opposites: Record<FlowAnchorSide, FlowAnchorSide> = {
  right: "left",
  left: "right",
  top: "bottom",
  bottom: "top",
};

it.each(["right", "left", "top", "bottom"] as const)(
  "%s 方向相邻对齐节点直接连接，不产生小回环",
  (side) => {
    const normal = ANCHOR_NORMALS[side];
    for (const gap of [40, 4, 12, 24, 30, 36, 48, 64]) {
      const target = {
        ...source,
        id: "target",
        x: normal.x * (source.width + gap),
        y: normal.y * (source.height + gap),
      };
      const from = rectAnchor(source, side, source.id);
      const to = rectAnchor(target, opposites[side], target.id);
      const path = routeEdge(from, to, new RoutingObstacles([source, target]));
      expect(path.status).toBe("routed");
      expect(path.points).toEqual([from.point, to.point]);
      expect(path.segments).toEqual([
        { kind: "line", from: from.point, to: to.point },
      ]);
    }
  },
);

it.each([1e-5, 1, 8, -8, 40])(
  "短连线错开 %s 时缩短两端引线，路径保持前进",
  (offset) => {
    const target = { ...source, id: "target", x: source.width + 40, y: offset };
    const from = rectAnchor(source, "right", source.id);
    const to = rectAnchor(target, "left", target.id);
    const path = routeEdge(from, to, new RoutingObstacles([source, target]));
    expect(path.status).toBe("routed");
    for (let i = 1; i < path.points.length; i++)
      expect(path.points[i].x).toBeGreaterThanOrEqual(path.points[i - 1].x);
    expect(path.bounds).toMatchObject({
      x: from.point.x,
      y: Math.min(from.point.y, to.point.y),
      width: 40,
    });
    expect(path.bounds.height).toBeCloseTo(Math.abs(offset), 10);
  },
);

it("直连仍检查无关障碍，重叠卡片仍明确受阻", () => {
  const target = { ...source, id: "target", x: 600 };
  const obstacle = { id: "obstacle", x: 340, y: 0, width: 100, height: 80 };
  const path = routeEdge(
    rectAnchor(source, "right", source.id),
    rectAnchor(target, "left", target.id),
    new RoutingObstacles([source, target, obstacle]),
  );
  expect(path.status).toBe("routed");
  expect(path.points.length).toBeGreaterThan(2);
  const overlap = { ...target, x: 200 };
  expect(
    routeEdge(
      rectAnchor(source, "right", source.id),
      rectAnchor(overlap, "left", overlap.id),
      new RoutingObstacles([source, overlap]),
    ).status,
  ).toBe("blocked");
});

it("轴向间距极短但纵向相隔较远时，仍可离开节点后正常绕行", () => {
  for (const gap of [4, 12, 20]) {
    const target = { ...source, id: "target", x: source.width + gap, y: 240 };
    const path = routeEdge(
      rectAnchor(source, "right", source.id),
      rectAnchor(target, "left", target.id),
      new RoutingObstacles([source, target]),
    );
    expect(path.status).toBe("routed");
  }
});
