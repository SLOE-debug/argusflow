import { describe, it, expect } from "vitest";
import {
  clickPoint,
  frameMap,
  reviewFrames,
  type ReviewFrame,
} from "../../../src/features/recorder";
import {
  action,
  clickAction,
  click,
  visual,
  visualRecord,
} from "./review-fixtures";
const frame: ReviewFrame = {
  id: 5,
  visual: visual("Input"),
  event: "Raw" in click.data ? click.data.Raw : null,
  eventId: 1,
};
describe("事件帧与二维坐标", () => {
  it("窗口切换显示已保存的后续菜单帧，不默认展示549ms的旧桌面", () => {
    const windowAction = {
      ...action,
      data: {
        Interaction: {
          raw: [7],
          kind: "WindowSwitch" as const,
          from_qpc: 992,
          through_qpc: 992,
          window: { handle: 1, pid: 7, epoch: 1 },
          basis: "foreground",
          related: null,
        },
      },
    };
    const records = [
      {
        id: 7,
        written_qpc: 992,
        data: {
          Raw: {
            ...("Raw" in click.data
              ? click.data.Raw
              : (() => {
                  throw Error("fixture");
                })()),
            qpc: 992,
            kind: "Context" as const,
          },
        },
      },
      visualRecord(
        44,
        visual("Input", { raw: [7], presented_ns: 549_000_000 }),
      ),
      visualRecord(
        54,
        visual("After", { raw: [7], presented_ns: 1_257_000_000 }),
      ),
    ];
    expect(reviewFrames(records, windowAction)[0].id).toBe(54);
  });
  it("文本输入优先独立响应，不用整组After的后续文字代替单字符", () => {
    const textAction = {
      ...clickAction,
      data: {
        Interaction: {
          raw: [1],
          kind: "TextUnconfirmed" as const,
          from_qpc: 100,
          through_qpc: 100,
          window: { handle: 1, pid: 7, epoch: 1 },
          basis: "key",
          related: null,
        },
      },
    };
    const frames = reviewFrames(
      [
        click,
        visualRecord(4, visual("Input")),
        visualRecord(5, visual("After")),
        visualRecord(6, visual("Response", { raw: [1] })),
      ],
      textAction,
    );
    expect(frames[0].id).toBe(6);
  });
  it("按下事件帧优先，补充帧独立保留，不把旧Before冒充事件帧", () => {
    const result = reviewFrames(
      [
        visualRecord(4, visual("After")),
        visualRecord(5, visual("Input")),
        click,
      ],
      clickAction,
    );
    expect(result.map((f) => f.id)).toEqual([5, 4]);
    expect(result[0].eventId).toBe(1);
    expect(
      reviewFrames([click, visualRecord(6, visual("Before"))], action)[0].event,
    ).toBeNull();
  });
  it("物理负坐标等比映射到OCR附件坐标，不重复乘DPI", () => {
    expect(clickPoint(frame)).toEqual({ x: 864, y: 540 });
    const packet = JSON.parse(
      frameMap(frame, {
        image_hash: "Input",
        image_size: [1728, 1080],
        blocks: [
          {
            text: "打开",
            confidence: 0.9,
            polygon: [
              [800, 500],
              [920, 500],
              [920, 570],
              [800, 570],
            ],
          },
        ],
      }),
    );
    expect(packet.click).toEqual({ x: 864, y: 540, marker_only: true });
    expect(packet.ocr[0].polygon[0]).toEqual([800, 500]);
    expect(packet.frame.hash).toBe("Input");
  });
  it("不在当前屏幕的点击不挤到边缘，也不使用另一张图的OCR", () => {
    expect(
      clickPoint({
        ...frame,
        visual: visual("Input", { screen_origin: [0, 0] }),
      }),
    ).toBeNull();
    const packet = JSON.parse(
      frameMap(frame, {
        image_hash: "other",
        image_size: [1728, 1080],
        blocks: [],
      }),
    );
    expect(packet.ocr).toBeNull();
  });
  it("敏感事件帧保留省略状态，不拿补充帧替换", () => {
    const result = reviewFrames(
      [
        click,
        visualRecord(
          5,
          visual("Input", { image: null, decision: "SensitiveOmitted" }),
        ),
        visualRecord(6, visual("After")),
      ],
      action,
    );
    expect(result[0].visual.image).toBeNull();
    expect(result[0].eventId).toBe(1);
  });
});
