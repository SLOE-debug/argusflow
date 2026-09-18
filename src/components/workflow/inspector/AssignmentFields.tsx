import {
  compileAssignment,
  formatExpression,
  formatAssignment,
  studio,
  updateNode,
  type Action,
  type EditorTab,
  type SymbolValue,
  type WorkflowNode,
} from "../../../features/workflow";
import { FormulaEditor } from "../value-editor/FormulaEditor";
import { TypeSelect } from "../value-editor/TypeSelect";
/** 变量声明与赋值以 name = expression 编辑，保存时仍使用强类型节点契约。 */
export function AssignmentFields({
  node,
  tab,
  symbols,
  action,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
  readonly symbols: readonly SymbolValue[];
  readonly action: Extract<Action, { readonly kind: "let" | "assign" }>;
}) {
  const entries =
    action.kind === "let"
      ? [{ name: action.name, value: action.value }]
      : action.assignments;
  return (
    <div className="space-y-3">
      {action.kind === "let" && (
        <TypeSelect
          value={action.value_type}
          onChange={(value_type) =>
            studio.changeNode(node.id, (current) => ({
              ...current,
              action: { ...action, value_type },
            }))
          }
        />
      )}
      {entries.map((entry, index) => {
        const field = action.kind === "let" ? "value" : "assignment." + index;
        return (
          <FormulaEditor
            key={
              field +
              (action.kind === "let" ? JSON.stringify(action.value_type) : "")
            }
            label="赋值语句"
            source={
              tab.file.editor.drafts[node.id + ":" + field] ??
              (action.kind === "assign"
                ? formatAssignment(entry.name, entry.value)
                : entry.name + " = " + formatExpression(entry.value))
            }
            symbols={symbols}
            readOnly={studio.readonly}
            compile={(source) =>
              compileAssignment(
                source,
                symbols,
                action.kind === "let" ? action.value_type : undefined,
              )
            }
            onInvalid={(source) => studio.draft(node.id, field, source)}
            onChange={(assignment) =>
              studio.draft(node.id, field, null, (file) =>
                updateNode(file, node.id, (current) => ({
                  ...current,
                  action:
                    action.kind === "let"
                      ? { ...action, ...assignment }
                      : {
                          ...action,
                          assignments: action.assignments.map((old, i) =>
                            i === index ? assignment : old,
                          ),
                        },
                })),
              )
            }
          />
        );
      })}
    </div>
  );
}
