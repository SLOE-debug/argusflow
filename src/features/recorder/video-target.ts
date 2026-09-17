import type { RecordingRecord, RawInput } from "./model";

/** 鼠标操作默认定位操作时刻；键盘操作默认定位结果。 */
export function isPointerAction(action: RecordingRecord) {
  if (!("Interaction" in action.data)) return false;
  const kind = action.data.Interaction.kind;
  return (
    kind === "Click" ||
    kind === "DoubleClick" ||
    kind === "Drag" ||
    kind === "Scroll"
  );
}

/** 点击目标必须取按下前的画面，不能由结果图或上一条键盘记录代替。 */
export function isClickAction(action: RecordingRecord) {
  return (
    "Interaction" in action.data &&
    (action.data.Interaction.kind === "Click" ||
      action.data.Interaction.kind === "DoubleClick")
  );
}

/** 只使用此操作关联的首个鼠标按下坐标，不猜测UIA控件中心。 */
export function clickPoint(
  action: RecordingRecord,
  records: readonly RecordingRecord[],
) {
  if (!("Interaction" in action.data) || !isClickAction(action)) return null;
  const ids = new Set(action.data.Interaction.raw);
  let first: RawInput | null = null;
  for (const record of records) {
    if (!ids.has(record.id) || !("Raw" in record.data)) continue;
    const raw = record.data.Raw;
    if (
      typeof raw.kind === "object" &&
      "Button" in raw.kind &&
      raw.kind.Button.down &&
      (!first || raw.qpc < first.qpc)
    )
      first = raw;
  }
  return first?.point ?? null;
}

/** 视频和Hook均使用物理像素；只减屏幕原点，不重复乘DPI。 */
export function imageClickPoint(
  point: RawInput["point"] | null,
  origin: readonly [number, number],
  size: readonly [number, number],
) {
  if (!point) return null;
  const x = point.x - origin[0];
  const y = point.y - origin[1];
  return x >= 0 && y >= 0 && x < size[0] && y < size[1] ? { x, y } : null;
}
