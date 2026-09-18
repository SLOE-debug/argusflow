import type { Expr, ValueType, WorkflowFile } from "./contracts";
import { defaultValue, literal } from "./expressions";
import { replaceScope, scopeById } from "./graph";

/** 更新公开结果时同时更新出口表达式，避免要求用户手动匹配类型。 */
export function setWorkflowResult(
  file: WorkflowFile,
  name: string,
  type: ValueType,
  value: Expr,
): WorkflowFile {
  const root = scopeById(file, file.definition.root);
  const drafts = { ...file.editor.drafts };
  delete drafts[root.id + ":output." + name];
  return replaceScope(
    {
      ...file,
      definition: {
        ...file.definition,
        outputs: { ...file.definition.outputs, [name]: type },
      },
      editor: { ...file.editor, drafts },
    },
    { ...root, outputs: { ...root.outputs, [name]: value } },
  );
}

/** 为输入或手动填写的结果设置类型；删除结果时同时清除出口和草稿。 */
export function updateWorkflowPort(
  file: WorkflowFile,
  field: "inputs" | "outputs",
  name: string,
  type: ValueType | null,
): WorkflowFile {
  if (field === "outputs" && type)
    return setWorkflowResult(
      file,
      name,
      type,
      literal(type, defaultValue(type)),
    );
  const fields = { ...file.definition[field] };
  if (type) fields[name] = type;
  else delete fields[name];
  const next = { ...file, definition: { ...file.definition, [field]: fields } };
  if (field === "inputs") return next;
  const root = scopeById(next, next.definition.root);
  const outputs = { ...root.outputs };
  const drafts = { ...next.editor.drafts };
  delete outputs[name];
  delete drafts[root.id + ":output." + name];
  return replaceScope(
    { ...next, editor: { ...next.editor, drafts } },
    { ...root, outputs },
  );
}

/** 修改公开名称，并保留结果内容及尚未完成的编辑。 */
export function renameWorkflowPort(
  file: WorkflowFile,
  field: "inputs" | "outputs",
  old: string,
  name: string,
): WorkflowFile {
  if (
    !name.trim() ||
    name === old ||
    Object.hasOwn(file.definition[field], name)
  )
    return file;
  const fields = Object.fromEntries(
    Object.entries(file.definition[field]).map(([key, type]) => [
      key === old ? name : key,
      type,
    ]),
  );
  const next = { ...file, definition: { ...file.definition, [field]: fields } };
  if (field === "inputs") return next;
  const root = scopeById(next, next.definition.root);
  const outputs = Object.fromEntries(
    Object.entries(root.outputs).map(([key, value]) => [
      key === old ? name : key,
      value,
    ]),
  );
  const drafts = { ...next.editor.drafts };
  const oldKey = root.id + ":output." + old;
  if (Object.hasOwn(drafts, oldKey)) {
    drafts[root.id + ":output." + name] = drafts[oldKey];
    delete drafts[oldKey];
  }
  return replaceScope(
    { ...next, editor: { ...next.editor, drafts } },
    { ...root, outputs },
  );
}

/** 保留可读名称，只在重名时添加编号。 */
export function nextPortName(
  fields: Readonly<Record<string, ValueType>>,
  preferred: string,
): string {
  let name = preferred;
  let index = 2;
  while (Object.hasOwn(fields, name)) name = preferred + index++;
  return name;
}
