import { expect, it } from "vitest";
import { SnapIndex, SNAP_DISTANCE, type FlowRect } from "../../../src/flow";
import { drawSnapGuides } from "../../../src/components/workflow/canvas/rendering/guides";
import { recordingContext } from "../support/canvas";

const moving: FlowRect = { x: 0, y: 0, width: 100, height: 60 };

it.each([
  ["left", { x: 197, y: 80 }, { x: 200, y: 80 }, "x", 200],
  ["center-x", { x: 252, y: 80 }, { x: 250, y: 80 }, "x", 300],
  ["right", { x: 304, y: 80 }, { x: 300, y: 80 }, "x", 400],
  ["top", { x: 80, y: 297 }, { x: 80, y: 300 }, "y", 300],
  ["center-y", { x: 80, y: 327 }, { x: 80, y: 330 }, "y", 360],
  ["bottom", { x: 80, y: 357 }, { x: 80, y: 360 }, "y", 420],
] as const)("支持 %s 对齐", (_, delta, expected, axis, position) => {
  const index = new SnapIndex([{ x: 200, y: 300, width: 200, height: 120 }]);
  const result = index.snap(moving, delta, 1);
  expect(result.delta).toEqual(expected);
  expect(result.guides).toHaveLength(1);
  expect(result.guides[0]).toMatchObject({ axis, position });
});

it.each([0.25, 0.7, 1, 4])(
  "缩放 %s 时吸附和脱离均为屏幕 5 像素，不累加偏移",
  (zoom) => {
    const index = new SnapIndex([{ x: 200, y: 300, width: 100, height: 60 }]);
    for (const pixels of [4, 5, 6, 4, -4, -6, 0, 6]) {
      const raw = { x: 200 + pixels / zoom, y: 70 };
      const result = index.snap(moving, raw, zoom);
      expect(result.delta.x).toBe(
        Math.abs(pixels) <= SNAP_DISTANCE ? 200 : raw.x,
      );
      expect(result.delta.y).toBe(70);
      expect(result.guides).toHaveLength(
        Math.abs(pixels) <= SNAP_DISTANCE ? 1 : 0,
      );
    }
  },
);

it("两轴独立吸附，参考线覆盖修正后的节点与对应目标", () => {
  const index = new SnapIndex([
    { x: 200, y: 500, width: 100, height: 60 },
    { x: 600, y: 300, width: 100, height: 60 },
  ]);
  expect(index.snap(moving, { x: 197, y: 296 }, 1)).toEqual({
    delta: { x: 200, y: 300 },
    guides: [
      { axis: "x", position: 250, start: 300, end: 560 },
      { axis: "y", position: 330, start: 200, end: 700 },
    ],
  });
});

it("同距离优先附近目标，候选顺序不影响结果", () => {
  const targets = [
    { x: 202, y: 900, width: 100, height: 60 },
    { x: 198, y: 300, width: 100, height: 60 },
  ];
  const first = new SnapIndex(targets).snap(moving, { x: 200, y: 100 }, 1);
  const second = new SnapIndex([...targets].reverse()).snap(
    moving,
    { x: 200, y: 100 },
    1,
  );
  expect(first).toEqual(second);
  expect(first.delta.x).toBe(198);
});

it("空目标和范围外保持亚像素自由移动，按下不发生位置跳变", () => {
  const delta = { x: 20.125, y: -7.625 };
  expect(new SnapIndex([]).snap(moving, delta, 1)).toEqual({
    delta,
    guides: [],
  });
  const index = new SnapIndex([{ x: 4, y: 500, width: 100, height: 60 }]);
  expect(index.snap(moving, { x: 0, y: 0 }, 1)).toEqual({
    delta: { x: 0, y: 0 },
    guides: [],
  });
  expect(index.snap(moving, delta, 1)).toEqual({ delta, guides: [] });
});

it.each([0.5, 1, 4])("辅助线在缩放 %s 时保持屏幕线宽与标记尺寸", (zoom) => {
  const { context, calls } = recordingContext();
  const strokes: { width: number; color: string }[] = [];
  calls.stroke.mockImplementation(() =>
    strokes.push({
      width: context.lineWidth,
      color: String(context.strokeStyle),
    }),
  );
  drawSnapGuides(
    context,
    [{ axis: "x", position: 100, start: 20, end: 200 }],
    { x: 30, y: 40, zoom },
    "#a044df",
  );
  expect(strokes).toEqual([
    { width: 1, color: "#a044df" },
    { width: 1, color: "#a044df" },
  ]);
  expect(calls.moveTo).toHaveBeenNthCalledWith(
    1,
    30 + 100 * zoom + 0.5,
    40 + 20 * zoom - 8 + 0.5,
  );
  expect(calls.setLineDash).toHaveBeenCalledWith([4, 3]);
  expect(calls.scale).not.toHaveBeenCalled();
  expect(calls.restore).toHaveBeenCalledOnce();
});
