import type { JsonValue, TaskDefinition } from "../model/contracts";

/** 节点显式选择识别平台，不从 AQL 或应用名称推断。 */
export const TARGET_PLATFORMS = ["uia", "cdp", "ocr"] as const;
export type TargetPlatform = (typeof TARGET_PLATFORMS)[number];
/** 范围的资源类型由平台决定，实际名称保存在 task.resources.scope。 */
export const TARGET_SCOPE_TYPES: Readonly<Record<TargetPlatform, string>> = {
  uia: "automation.window",
  cdp: "automation.page",
  ocr: "automation.query_source",
};
/** 所有需要目标定位的动作使用同一配置入口。 */
export const TARGET_TASKS = [
  "aql.query",
  "aql.preview",
  "aql.exists",
  "aql.wait",
  "aql.click",
  "aql.type_text",
  "aql.press_keys",
] as const;
export function isTargetTask(id: string): boolean {
  return TARGET_TASKS.some((kind) => kind === id);
}
export function targetPlatform(
  config: Readonly<Record<string, JsonValue>>,
): TargetPlatform | null {
  return (
    TARGET_PLATFORMS.find((platform) => platform === config.platform) ?? null
  );
}
/** 切换平台时清除范围，防止旧窗口或页面被误用于另一平台。 */
export function changeTargetPlatform(
  task: TaskDefinition,
  platform: TargetPlatform,
): TaskDefinition {
  return {
    ...task,
    config: { ...task.config, platform },
    resources: { scope: "" },
  };
}
/** 范围端口与后端 TaskSignature 共用相同平台约束。 */
export function targetResources(
  task: TaskDefinition,
): Readonly<Record<string, string>> {
  const platform = targetPlatform(task.config);
  return platform ? { scope: TARGET_SCOPE_TYPES[platform] } : {};
}
