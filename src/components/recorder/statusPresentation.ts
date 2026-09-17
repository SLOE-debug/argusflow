import { phaseLabels, type RecorderStatus } from "../../features/recorder";

/** 单一标题栏状态；证据缺口为警告，输入或会话故障才使用错误样式。 */
export function recorderIndicator(
  status: RecorderStatus | null,
  error: string | null,
  busy: boolean,
): {
  readonly label: string;
  readonly tone: "normal" | "warning" | "error";
  readonly detail: string;
} {
  const phase = status?.phase;
  const state = phase ? phaseLabels[phase] : "尚未录制";
  const fault = error || status?.input_fault;
  if (fault || phase === "Faulted" || phase === "Interrupted")
    return {
      label: "录制异常",
      tone: "error",
      detail: `${state}${fault ? ` · ${fault}` : ""}；在菜单中查看录制详情`,
    };
  if (status?.evidence_gaps)
    return {
      label: "证据不完整",
      tone: "warning",
      detail: `${state} · ${status.evidence_gaps} 条证据缺失记录；在菜单中查看录制详情`,
    };
  return {
    label: busy ? "正在处理…" : phase && phase !== "Stopped" ? state : "录制",
    tone: "normal",
    detail: state,
  };
}
