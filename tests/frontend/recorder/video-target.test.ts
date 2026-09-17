import { describe, expect, it } from "vitest";
import {
  clickPoint,
  imageClickPoint,
} from "../../../src/features/recorder/video-target";
import { click, clickAction } from "./review-fixtures";

describe("点击目标证据", () => {
  it("只取关联的鼠标按下记录，缺失时不猜测坐标", () => {
    expect(clickPoint(clickAction, [click])).toEqual({ x: -1280, y: 800 });
    expect(clickPoint(clickAction, [{ ...click, id: 100 }])).toBeNull();
    if (!("Raw" in click.data)) throw Error("fixture");
    const released = {
      ...click,
      data: {
        Raw: {
          ...click.data.Raw,
          kind: { Button: { button: "Left" as const, down: false } },
        },
      },
    };
    expect(clickPoint(clickAction, [released])).toBeNull();
  });
  it("屏幕外的坐标不能被挤到图片边缘冒充命中", () => {
    expect(
      imageClickPoint({ x: 0, y: 800 }, [-2560, 0], [2560, 1600]),
    ).toBeNull();
    expect(
      imageClickPoint({ x: -1280, y: 800 }, [-2560, 0], [2560, 1600]),
    ).toEqual({ x: 1280, y: 800 });
  });
});
