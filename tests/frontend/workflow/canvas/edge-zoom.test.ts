import { expect, it } from "vitest";
import { drawEdge } from "../../../../src/components/workflow/canvas/rendering/edges";
import { hitTest } from "../../../../src/components/workflow/canvas/hitTest";
import { presentNodes } from "../../../../src/components/workflow/canvas/scene";
import {
  buildCanvasScene,
  createWorkflow,
} from "../../../../src/features/workflow";
import { routeEdge, RoutingObstacles } from "../../../../src/flow";
import { recordingContext } from "../../support/canvas";

const path = routeEdge(
  { point: { x: 0, y: 0 }, side: "right" },
  { point: { x: 200, y: 0 }, side: "left" },
  new RoutingObstacles([]),
);

it.each([
  [0.125, 0.2125, 0.75],
  [0.5, 0.85, 3],
  [1, 1.7, 6],
  [4, 1.7, 6],
])("缩放 %s 时线宽与箭头按可见比例绘制", (zoom, width, arrowSize) => {
  const { context, calls } = recordingContext();
  const widths: number[] = [];
  /** 绘制上下文由场景先应用局部缩放，记录最终 CSS 像素宽度。 */
  calls.stroke.mockImplementation(() => {
    widths.push(context.lineWidth * zoom);
  });
  drawEdge(context, path, "#2563eb", zoom);
  expect(widths).toEqual([width, width]);
  const arrowStart = calls.moveTo.mock.calls.at(-1)!;
  expect(Math.hypot(200 - arrowStart[0], arrowStart[1]) * zoom).toBeCloseTo(
    arrowSize,
  );
});

it.each([
  [true, false, 0.7, 0.22],
  [false, true, 0.9, 0.25],
] as const)(
  "远景下 hover=%s selected=%s 的底衬也同步缩小",
  (hover, selected, glow, width) => {
    const { context, calls } = recordingContext();
    const widths: number[] = [];
    calls.stroke.mockImplementation(() => {
      widths.push(context.lineWidth * 0.1);
    });
    drawEdge(context, path, "#2563eb", 0.1, hover, selected);
    expect(widths).toHaveLength(3);
    expect(widths[0]).toBeCloseTo(glow);
    expect(widths[1]).toBeCloseTo(width);
    expect(widths[2]).toBeCloseTo(width);
  },
);

it("受阻线路的虚线间隔与线条一起缩小", () => {
  const { context, calls } = recordingContext();
  drawEdge(context, { ...path, status: "blocked" }, "#d13c54", 0.1);
  expect(
    calls.setLineDash.mock.calls[0][0].map((length: number) => length * 0.1),
  ).toEqual([0.5, 0.4]);
});

it.each([0.125, 0.5, 1, 4])("缩放 %s 时细线仍保留屏幕像素命中范围", (zoom) => {
  const file = createWorkflow();
  const scene = buildCanvasScene(file);
  const nodes = presentNodes(file);
  const edge = scene.details[scene.root].edges[0];
  const x = (edge.sourceAnchor.point.x + edge.targetAnchor.point.x) / 2;
  const y = edge.sourceAnchor.point.y;
  const camera = { x: 40, y: 30, zoom };
  const screen = { x: x * zoom + camera.x, y: y * zoom + camera.y };
  expect(
    hitTest(scene, nodes, camera, { ...screen, y: screen.y + 7 }),
  ).toMatchObject({ kind: "edge", edge: edge.id });
  expect(
    hitTest(scene, nodes, camera, { ...screen, y: screen.y + 9 }).kind,
  ).toBe("background");
});
