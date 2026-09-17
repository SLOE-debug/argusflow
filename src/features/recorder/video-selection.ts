import type { RecordingRecord } from "./model";
import { markerLabel } from "./playback";

/** 点击最多分析2秒内的像素变化；逐键仍用300ms，所有结果受下一次主动操作约束。 */
export function resultFrameQuery(
  action: RecordingRecord,
  records: readonly RecordingRecord[],
  frequency: number,
) {
  const interaction =
    "Interaction" in action.data ? action.data.Interaction : null;
  const anchor = BigInt(
    interaction?.through_qpc ??
      ("Raw" in action.data ? action.data.Raw.qpc : action.written_qpc),
  );
  const click =
    interaction?.kind === "Click" || interaction?.kind === "DoubleClick";
  const delay = BigInt(Math.round(frequency * 0.3));
  let target = anchor + BigInt(Math.round(frequency * (click ? 2 : 0.3)));
  for (const record of records) {
    if (
      !("Interaction" in record.data) ||
      record.data.Interaction.kind === "WindowSwitch"
    )
      continue;
    const next = BigInt(record.data.Interaction.from_qpc);
    if (next > anchor && next <= target) target = next - 1n;
  }
  if (!click) return { qpc: target.toString(), direction: 0 };
  let earliest = anchor + delay;
  for (const record of records) {
    if (
      !("Interaction" in record.data) ||
      record.data.Interaction.kind !== "WindowSwitch"
    )
      continue;
    const switched = BigInt(record.data.Interaction.through_qpc);
    if (switched > anchor && switched <= target && switched + delay > earliest)
      earliest = switched + delay;
  }
  return {
    qpc: target.toString(),
    direction: 0,
    settleAfter: (earliest < target ? earliest : target).toString(),
  };
}

/** 虚拟键仅作为原始按键提示，不把它冒充布局/IME确认后的输入文本。 */
export function inputKeyLabel(
  action: RecordingRecord,
  records: readonly RecordingRecord[],
) {
  if ("Raw" in action.data) {
    const kind = action.data.Raw.kind;
    return typeof kind === "object" && "Key" in kind
      ? markerLabel({
          id: action.id,
          time_ms: 0,
          input: { kind: "key", vk: kind.Key.vk },
        })
      : "";
  }
  if (
    !("Interaction" in action.data) ||
    action.data.Interaction.kind !== "TextUnconfirmed"
  )
    return "";
  const raw = new Set(action.data.Interaction.raw);
  const keys = records.flatMap((record) => {
    if (!raw.has(record.id) || !("Raw" in record.data)) return [];
    const kind = record.data.Raw.kind;
    return typeof kind === "object" &&
      "Key" in kind &&
      kind.Key.down &&
      kind.Key.vk >= 65 &&
      kind.Key.vk <= 90
      ? [String.fromCharCode(kind.Key.vk)]
      : [];
  });
  return keys.length ? `按键 ${keys.at(-1)}` : "";
}
