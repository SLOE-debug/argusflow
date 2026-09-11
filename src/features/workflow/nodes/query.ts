import type { ValueType, WorkflowFile } from "../model/contracts";
import { defaultValue, literal } from "../model/expressions";
import { updateNode } from "../model/graph";

/** 查询、参数端口和失效草稿作为同一次编辑提交，撤销可完整恢复。 */
export function applyQuery(
  file: WorkflowFile,
  nodeId: string,
  query: string,
  fields: Readonly<Record<string, ValueType>>,
): WorkflowFile {
  const drafts = { ...file.editor.drafts };
  delete drafts[nodeId + ":aql"];
  const prefix = nodeId + ":input.";
  for (const key of Object.keys(drafts)) {
    if (
      key.startsWith(prefix) &&
      !Object.hasOwn(fields, key.slice(prefix.length))
    ) {
      delete drafts[key];
    }
  }
  const next = updateNode(file, nodeId, (node) => {
    if (node.action.kind !== "task") return node;
    const task = node.action.task;
    return {
      ...node,
      action: {
        ...node.action,
        task: {
          ...task,
          config: { ...task.config, query },
          inputs: Object.fromEntries(
            Object.entries(fields).map(([name, type]) => [
              name,
              task.inputs[name] ?? literal(type, defaultValue(type)),
            ]),
          ),
        },
      },
    };
  });
  return { ...next, editor: { ...next.editor, drafts } };
}
