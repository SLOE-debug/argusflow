import { expect, it } from "vitest";
import { drawBackground } from "../../../../src/components/workflow/canvas/rendering/background";
import {
  LIGHT_THEME,
  DARK_THEME,
} from "../../../../src/features/themes/catalog";
import { recordingContext } from "../../support/canvas";

it("四倍缩放换档前后点阵位置、大小和透明度连续交接", () => {
  const record = (zoom: number) => {
    const { context, calls } = recordingContext();
    const layers: { alpha: number; dots: number[][] }[] = [];
    calls.beginPath.mockImplementation(() => calls.arc.mockClear());
    calls.fill.mockImplementation(() =>
      layers.push({
        alpha: context.globalAlpha,
        dots: calls.arc.mock.calls.map((args) => args.slice(0, 3)),
      }),
    );
    drawBackground(
      context,
      { x: 7, y: -11, zoom },
      300,
      170,
      LIGHT_THEME.colors,
    );
    return layers;
  };
  const before = record(4 - 1e-8),
    after = record(4);
  expect(before).toHaveLength(after.length);
  before.forEach((layer, index) => {
    expect(layer.alpha).toBeCloseTo(after[index].alpha, 6);
    expect(layer.dots).toHaveLength(after[index].dots.length);
    layer.dots.forEach((point, dot) =>
      point.forEach((value, axis) => {
        expect(value).toBeCloseTo(after[index].dots[dot][axis], 5);
      }),
    );
  });
});

it("深浅主题使用各自纸面颜色，极近极远及负坐标平移只绘制有限屏幕点阵", () => {
  for (const theme of [LIGHT_THEME, DARK_THEME])
    for (const zoom of [1e-6, 0.55, 1, 3.9, 100, 1e16]) {
      const { context, calls, gradient } = recordingContext();
      drawBackground(
        context,
        { x: -1e7, y: 2e7, zoom },
        800,
        600,
        theme.colors,
      );
      expect(gradient.addColorStop).toHaveBeenCalledWith(
        0,
        theme.colors.surface,
      );
      expect(gradient.addColorStop).toHaveBeenCalledWith(
        1,
        theme.colors.canvas,
      );
      expect(calls.arc.mock.calls.length).toBeGreaterThan(0);
      expect(calls.arc.mock.calls.length).toBeLessThan(10000);
      expect(
        calls.arc.mock.calls.every((args) => args.every(Number.isFinite)),
      ).toBe(true);
      expect(calls.fillStyle).toBe(theme.colors.grid);
      expect(calls.save).toHaveBeenCalledTimes(1);
      expect(calls.restore).toHaveBeenCalledTimes(1);
    }
});
