import type { InteractionKind, Phase, RecordingRecord } from "./model";
export const phaseLabels: Readonly<Record<Phase, string>> = {
  Recording: "正在录制",
  Paused: "已暂停",
  Stopping: "正在保存",
  Stopped: "已停止",
  Faulted: "录制故障",
  Interrupted: "录制中断",
};
export const actionLabels: Readonly<Record<InteractionKind, string>> = {
  Click: "点击",
  DoubleClick: "双击",
  Drag: "拖拽",
  Scroll: "滚动",
  Chord: "组合键",
  SystemKey: "系统按键",
  TextUnconfirmed: "文本输入 · 结果未确认",
  ImeUnconfirmed: "输入法编辑 · 结果未确认",
  PasteUnconfirmed: "粘贴 · 结果未确认",
  DeleteUnconfirmed: "删除 · 结果未确认",
  TextObserved: "观察到编辑值",
  WindowSwitch: "窗口切换",
  Unresolved: "未能识别的操作",
};
/** 以原始来源引用关联操作、结构和视觉，不猜测因果关系。 */
export function evidenceFor(
  records: readonly RecordingRecord[],
  action: RecordingRecord,
): readonly RecordingRecord[] {
  if (!("Interaction" in action.data) && !("Raw" in action.data)) return [];
  const ids = new Set(
    "Interaction" in action.data ? action.data.Interaction.raw : [action.id],
  );
  const related =
    "Interaction" in action.data ? action.data.Interaction.related : null;
  if (related !== null) {
    ids.add(related);
    for (const { data } of records) {
      if ("Interaction" in data && data.Interaction.raw.includes(related))
        data.Interaction.raw.forEach((id) => ids.add(id));
    }
  }
  return records.filter(({ id, data }) =>
    "Raw" in data
      ? ids.has(id)
      : "Structure" in data
        ? data.Structure.raw.some((ref) => ids.has(ref))
        : "Visual" in data
          ? data.Visual.raw.some((ref) => ids.has(ref))
          : "Ocr" in data
            ? data.Ocr.raw.some((ref) => ids.has(ref))
            : "Attempt" in data
              ? data.Attempt.raw.some((ref) => ids.has(ref))
              : "Association" in data
                ? data.Association.records.some((ref) => ids.has(ref))
                : false,
  );
}
/** 双击保留第一次点击为来源，在时间线中只显示最终组合项。 */
export function timeline(
  records: readonly RecordingRecord[],
): readonly RecordingRecord[] {
  const related = new Set(
    records.flatMap(({ data }) =>
      "Interaction" in data && data.Interaction.related !== null
        ? [data.Interaction.related]
        : [],
    ),
  );
  return records.filter(
    ({ data }) =>
      "Interaction" in data &&
      !(
        data.Interaction.kind === "Click" &&
        data.Interaction.raw.some((id) => related.has(id))
      ),
  );
}
