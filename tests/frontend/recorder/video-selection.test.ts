import { describe, expect, it } from "vitest";
import { resultFrameQuery } from "../../../src/features/recorder/video-selection";
import { action } from "./review-fixtures";
import type {
  InteractionKind,
  RecordingRecord,
} from "../../../src/features/recorder/model";

function event(
  id: number,
  time: number,
  kind: InteractionKind = "TextUnconfirmed",
): RecordingRecord {
  if (!("Interaction" in action.data)) throw Error("fixture");
  return {
    id,
    written_qpc: time,
    data: {
      Interaction: {
        ...action.data.Interaction,
        kind,
        from_qpc: time,
        through_qpc: time,
      },
    },
  };
}
describe("操作结果取帧边界", () => {
  it("启动点击跟随后续窗口通知，等待区间不能超过下一次主动操作", () => {
    const records = [
      event(1, 4156, "Click"),
      event(2, 4253, "WindowSwitch"),
      event(3, 4579, "WindowSwitch"),
      event(4, 6861, "Click"),
    ];
    expect(resultFrameQuery(records[0], records, 1000)).toEqual({
      qpc: "6156",
      direction: 0,
      settleAfter: "4879",
    });
    const next = event(5, 5000);
    expect(resultFrameQuery(records[0], [...records, next], 1000).qpc).toBe(
      "4999",
    );
  });
  it("快速n/o/t每个操作独立选帧，结果不能跨入下一次按键", () => {
    const records = [
      event(1, 1209),
      event(2, 1410),
      event(3, 1618),
      event(4, 1964),
    ];
    expect(
      records.slice(0, 3).map((a) => resultFrameQuery(a, records, 1000)),
    ).toEqual([
      { qpc: "1409", direction: 0 },
      { qpc: "1617", direction: 0 },
      { qpc: "1918", direction: 0 },
    ]);
  });
  it("窗口通知留出绘制时间，Win键的结果不被紧随其后的窗口通知截断", () => {
    const records = [
      event(1, 453, "SystemKey"),
      event(2, 456, "WindowSwitch"),
      event(3, 1209),
    ];
    expect(resultFrameQuery(records[0], records, 1000).qpc).toBe("753");
    expect(resultFrameQuery(records[1], records, 1000).qpc).toBe("756");
  });
});
