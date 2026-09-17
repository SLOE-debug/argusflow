import { describe, expect, it } from "vitest";
import { appendWindow } from "../../../src/features/recorder/buffer";
import {
  evidenceFor,
  timeline,
  type RecordingRecord,
} from "../../../src/features/recorder";
const click: RecordingRecord = {
  id: 3,
  written_qpc: 30,
  data: {
    Interaction: {
      kind: "Click",
      raw: [1, 2],
      from_qpc: 10,
      through_qpc: 20,
      window: { handle: 1, pid: 2, epoch: 1 },
      related: null,
      basis: "test",
    },
  },
};
describe("录制时间线", () => {
  it("页面证据不生成重复操作，关联只使用原始记录", () => {
    const attempt: RecordingRecord = {
      id: 4,
      written_qpc: 40,
      data: {
        Attempt: {
          raw: [1],
          stage: "Cdp",
          outcome: "Unavailable",
          reason: "未绑定",
        },
      },
    };
    const unrelated: RecordingRecord = {
      id: 5,
      written_qpc: 50,
      data: {
        Attempt: {
          raw: [100],
          stage: "Uia",
          outcome: "Unavailable",
          reason: "目标不同",
        },
      },
    };
    expect(timeline([click, attempt, unrelated])).toEqual([click]);
    expect(evidenceFor([click, attempt, unrelated], click)).toEqual([attempt]);
  });
  it("双击保留原记录但只显示组合操作", () => {
    if (!("Interaction" in click.data)) throw Error("fixture");
    const double: RecordingRecord = {
      id: 6,
      written_qpc: 60,
      data: {
        Interaction: {
          ...click.data.Interaction,
          kind: "DoubleClick",
          raw: [4, 5],
          related: 2,
        },
      },
    };
    expect(timeline([click, double])).toEqual([double]);
    const firstEvidence: RecordingRecord = {
      id: 7,
      written_qpc: 70,
      data: {
        Attempt: {
          raw: [1],
          stage: "Uia",
          outcome: "Unavailable",
          reason: "first",
        },
      },
    };
    expect(evidenceFor([click, firstEvidence, double], double)).toEqual([
      firstEvidence,
    ]);
  });
  it("实时缓存有限，原始数组不修改", () => {
    const records = Array.from({ length: 5000 }, (_, id) => ({ ...click, id }));
    const window = appendWindow([], records);
    expect(window).toHaveLength(4096);
    expect(window[0].id).toBe(904);
    expect(records).toHaveLength(5000);
    const large = {
      ...click,
      data: {
        State: {
          phase: "Paused" as const,
          reason: "x".repeat(5 * 1024 * 1024),
        },
      },
    };
    expect(appendWindow([large], [large])).toHaveLength(1);
  });
});
