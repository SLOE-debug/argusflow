import type { AriaAttributes } from "react";

/** ARIA 的字符串 false 必须视为正常状态。 */
export function isInvalid(value: AriaAttributes["aria-invalid"]): boolean {
  return Boolean(value && value !== "false");
}

/** 基础控件共享密度，单位由 Tailwind 间距定义。 */
export type ControlSize = "default" | "compact";
/** 表单外壳统一交互状态；颜色只引用主题语义。 */
export const CONTROL_STYLE =
  "min-w-0 rounded-md border border-line bg-surface text-xs text-ink shadow-xs transition-colors hover:border-strong focus-within:border-accent/60 has-[:disabled]:cursor-not-allowed has-[:disabled]:bg-subtle has-[:disabled]:text-muted data-[invalid=true]:border-danger";
/** 紧凑规格用于面板与工具栏。 */
export const CONTROL_HEIGHT: Readonly<Record<ControlSize, string>> = {
  default: "h-8",
  compact: "h-7",
};
