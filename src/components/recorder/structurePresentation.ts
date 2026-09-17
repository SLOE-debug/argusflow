import type {
  Property,
  RecordingRecord,
  Structure,
} from "../../features/recorder/model";

/** 仅合并显示内容相同的快照，保留全部原始记录编号。 */
export function structureGroups(records: readonly RecordingRecord[]) {
  const groups = new Map<
    string,
    { structure: Structure; records: RecordingRecord[] }
  >();
  for (const record of records) {
    if (!("Structure" in record.data)) continue;
    const {
      raw: _raw,
      from_qpc: _from,
      through_qpc: _through,
      ...display
    } = record.data.Structure;
    const key = JSON.stringify(display);
    const group = groups.get(key);
    if (group) group.records.push(record);
    else
      groups.set(key, { structure: record.data.Structure, records: [record] });
  }
  return [...groups.values()];
}
const labels: Record<string, string> = {
  name: "控件名称",
  observed_value: "读取到的内容",
  enabled: "可用",
  focused: "接收键盘输入",
  password: "密码输入框",
  role: "控件类型编号",
  automation_id: "自动化标识",
  class_name: "窗口类名",
  hwnd_hint: "窗口句柄",
  pid: "进程编号",
  foreground_epoch: "前台窗口版本",
};
export const fieldLabel = (name: string) => labels[name] ?? name;
export function fieldValue(value: Property) {
  if (value === true) return "是";
  if (value === false) return "否";
  if (value === null || value === "") return "未提供";
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}
export function controlType(structure: Structure) {
  if (structure.source !== "Uia") return "控件";
  return (
    ({ 50000: "按钮", 50004: "输入框" } as Record<string, string>)[
      String(structure.properties.role)
    ] ?? "控件"
  );
}
