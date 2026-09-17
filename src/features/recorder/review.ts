import type { RawInput, RecordingRecord, Session, Visual } from "./model";
import { actionLabels } from "./selectors";

/** 一条可回看的帧引用；事件帧与补充采样统一按时间浏览。 */
export interface ReviewFrame {
  readonly id: number;
  readonly visual: Visual;
  readonly event: RawInput | null;
  readonly eventId: number | null;
}
/** 点击保留定位帧；文字/窗口切换优先响应，不能用同一排序替代不同操作语义。 */
export function reviewFrames(
  evidence: readonly RecordingRecord[],
  action: RecordingRecord,
): readonly ReviewFrame[] {
  const raw = evidence.filter((r) => "Raw" in r.data);
  const own = "Interaction" in action.data ? action.data.Interaction.raw : [];
  const kind =
    "Interaction" in action.data ? action.data.Interaction.kind : "Unresolved";
  const pointer = ["Click", "DoubleClick", "Drag"].includes(kind);
  const window = kind === "WindowSwitch" || kind === "SystemKey";
  const rank = (frame: ReviewFrame) => {
    if (frame.visual.decision === "SensitiveOmitted") return -1;
    if (pointer) return frame.visual.relation === "Input" ? 0 : 1;
    if (frame.visual.relation === "Response") return 0;
    if (window && frame.visual.relation === "After") return 1;
    return frame.visual.relation === "Input" ? 2 : 3;
  };
  const priority = (r: RecordingRecord) => {
    if (!("Raw" in r.data)) return 2;
    const kind = r.data.Raw.kind;
    return typeof kind === "object" &&
      (("Key" in kind && kind.Key.down) ||
        ("Button" in kind && kind.Button.down))
      ? 0
      : 1;
  };
  const events = raw
    .filter((r) => own.includes(r.id))
    .sort((a, b) => priority(a) - priority(b) || a.id - b.id);
  const result: ReviewFrame[] = [];
  for (const { id, data } of evidence) {
    if (!("Visual" in data)) continue;
    const visual = data.Visual;
    const event =
      visual.relation === "Input"
        ? events.find((r) => visual.raw.includes(r.id))
        : undefined;
    result.push({
      id,
      visual,
      event: event && "Raw" in event.data ? event.data.Raw : null,
      eventId: event?.id ?? null,
    });
  }
  return result.sort((a, b) => {
    const ai =
      a.eventId === null
        ? events.length
        : events.findIndex((r) => r.id === a.eventId);
    const bi =
      b.eventId === null
        ? events.length
        : events.findIndex((r) => r.id === b.eventId);
    return (
      rank(a) - rank(b) ||
      ai - bi ||
      a.visual.through_ns - b.visual.through_ns ||
      a.id - b.id
    );
  });
}
/** 回看列表用短标题，识别依据保留在记录详情中。 */
export function reviewTitle(action: RecordingRecord): string {
  return "Interaction" in action.data
    ? actionLabels[action.data.Interaction.kind].split(" · ")[0]
    : "Raw" in action.data
      ? "原始输入"
      : "操作";
}
/** 只取已观测到的目标名称，不根据键码推断应用或输入值。 */
export function reviewTargets(
  records: readonly RecordingRecord[],
): ReadonlyMap<number, string> {
  const targets = new Map<number, string>();
  for (const { data } of records) {
    if (!("Structure" in data) || data.Structure.stale) continue;
    const name = data.Structure.properties.name;
    if (typeof name === "string" && name.trim())
      for (const id of data.Structure.raw) targets.set(id, name);
  }
  return targets;
}
/** 从预先建立的原始输入索引读取名称，避免每个步骤重复扫描整份日志。 */
export function reviewTarget(
  targets: ReadonlyMap<number, string>,
  action: RecordingRecord,
): string {
  if (!("Interaction" in action.data)) return "";
  return (
    action.data.Interaction.raw
      .map((id) => targets.get(id))
      .filter(Boolean)
      .at(-1) ?? ""
  );
}
/** 用会话时钟显示操作时间，保留毫秒便于区分快速输入。 */
export function reviewTime(action: RecordingRecord, session: Session): string {
  if (!("Interaction" in action.data)) return "";
  const ms = Math.max(
    0,
    Math.floor(
      ((action.data.Interaction.from_qpc - session.qpc_origin) * 1000) /
        session.qpc_frequency,
    ),
  );
  return `${Math.floor(ms / 60000)
    .toString()
    .padStart(2, "0")}:${Math.floor((ms / 1000) % 60)
    .toString()
    .padStart(2, "0")}.${(ms % 1000).toString().padStart(3, "0")}`;
}
