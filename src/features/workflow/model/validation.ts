import type { Problem, WorkflowFile } from "./contracts";
import { endpointId } from "./endpoints";

/** 保存结构错误保留作用域位置，供问题面板定位。 */
export class SaveStructureError extends Error {
  constructor(readonly problems: readonly Problem[]) {
    super(problems.map((problem) => problem.message).join("；"));
    this.name = "SaveStructureError";
  }
}
/** 完整文件需要每个作用域的起止；允许连线和业务配置尚未完成。 */
export function assertSaveable(file: WorkflowFile): void {
  const problems: Problem[] = [];
  for (const scope of file.definition.scopes)
    for (const kind of ["start", "end"] as const) {
      if (!file.editor.nodes[endpointId(scope.id, kind)])
        problems.push({
          code: "missing_endpoint",
          workflow: file.id,
          scope: scope.id,
          node: null,
          message:
            "此作用域缺少" +
            (kind === "start" ? "开始" : "结束") +
            "，请添加后保存",
        });
    }
  if (problems.length) throw new SaveStructureError(problems);
}
