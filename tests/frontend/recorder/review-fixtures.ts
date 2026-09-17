import type { RecordingRecord, Visual } from "../../../src/features/recorder";
export const action: RecordingRecord = {
  id: 3,
  written_qpc: 100,
  data: {
    Interaction: {
      raw: [1, 2],
      kind: "SystemKey",
      from_qpc: 100,
      through_qpc: 120,
      window: { handle: 1, pid: 7, epoch: 1 },
      basis: "Win键释放",
      related: null,
    },
  },
};
export const clickAction: RecordingRecord = {
  ...action,
  data: {
    Interaction: {
      ...("Interaction" in action.data
        ? action.data.Interaction
        : (() => {
            throw Error("fixture");
          })()),
      kind: "Click",
    },
  },
};
export function visual(
  relation: Visual["relation"],
  overrides: Partial<Visual> = {},
): Visual {
  return {
    raw: [1, 2],
    relation,
    version: {
      session: "1",
      source: 1,
      generation: 1,
      revision: relation === "Before" ? 1 : 2,
    },
    presented_ns: 100,
    acquired_ns: 101,
    frozen_ns: 102,
    from_ns: 100,
    through_ns: relation === "Before" ? 100 : 500,
    region: [0, 0, 2560, 1600],
    screen_origin: [-2560, 0],
    dpi: [144, 144],
    status:
      relation === "Before" || relation === "Input" ? "Anchored" : "Observed",
    decision: { VisualRequired: "test" },
    image: {
      hash: relation,
      path: `attachments/${relation}.png`,
      bytes: 100,
      size: [1728, 1080],
    },
    ...overrides,
  };
}
export function visualRecord(id: number, value: Visual): RecordingRecord {
  return { id, written_qpc: id, data: { Visual: value } };
}
/** 负原点、150%DPI屏幕中心的实际鼠标按下事件。 */
export const click: RecordingRecord = {
  id: 1,
  written_qpc: 100,
  data: {
    Raw: {
      sequence: 1,
      qpc: 100,
      point: { x: -1280, y: 800 },
      window: { handle: 1, pid: 7, epoch: 1 },
      foreground_handle: 1,
      origin: "System",
      flags: 0,
      system_time: 0,
      kind: { Button: { button: "Left", down: true } },
    },
  },
};
