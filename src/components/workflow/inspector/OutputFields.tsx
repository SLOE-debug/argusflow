import { ChevronRight, Plus, Trash2 } from "lucide-react";
import {
  availableSymbols,
  childScopes,
  defaultValue,
  inferExpression,
  literal,
  replaceScope,
  scopeById,
  studio,
  taskSpec,
  updateNode,
  type EditorTab,
  type Expr,
  type SymbolValue,
  type WorkflowNode,
} from "../../../features/workflow";
import { Button, Input } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { TypeSelect } from "../value-editor/TypeSelect";
import { useDocuments } from "./useDocuments";
import { scopeCanComplete } from "../../../features/workflow/model/outputs";

/** 容器出口按每个分支配置；节点附加输出只映射执行结果，不修改资源所有权。 */
export function OutputFields({
  node,
  tab,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
}) {
  const documents = useDocuments();
  if (["return", "break", "continue", "fail"].includes(node.action.kind))
    return null;
  const symbols = availableSymbols(tab.file, tab.scope, node.id, documents);
  const native =
    node.action.kind === "task"
      ? taskSpec(node.action.task.type_id)?.outputs
      : node.action.kind === "call_workflow"
        ? documents.find(
            (file) =>
              node.action.kind === "call_workflow" &&
              file.id === node.action.workflow,
          )?.definition.outputs
        : undefined;
  const results: SymbolValue[] = Object.entries(native ?? {}).map(
    ([output, type]) => ({
      label: "当前结果 · " + output,
      type,
      expression: { kind: "result", output },
    }),
  );
  const children = ["while", "for_each"].includes(node.action.kind)
    ? []
    : childScopes(node.action);
  return (
    <details className="group mt-5 border-t border-line pt-3">
      <summary className="flex cursor-pointer list-none items-center gap-2 text-xs">
        <ChevronRight size={12} className="group-open:rotate-90" />
        输出
        <span className="ml-auto text-[10px] text-muted">
          {Object.keys(native ?? {}).length +
            Object.keys(node.output_bindings).length}{" "}
          项
        </span>
      </summary>
      {native && (
        <p className="mt-3 text-[11px] leading-5 text-muted">
          可直接引用：{Object.keys(native).join("、") || "无原生数据输出"}
        </p>
      )}
      {children
        .filter((child) => scopeCanComplete(tab.file, child.id))
        .map((child) => (
          <section className="mt-4" key={child.id}>
            <h4 className="mb-2 text-[11px] text-muted">
              {child.label} · 正常出口
            </h4>
            <OutputList
              values={scopeById(tab.file, child.id).outputs}
              symbols={
                availableSymbols(tab.file, child.id, undefined, documents)
                  .values
              }
              onChange={(values, name) =>
                studio.draft(
                  node.id,
                  "scope-output." + child.id + "." + name,
                  null,
                  (file) =>
                    replaceScope(file, {
                      ...scopeById(file, child.id),
                      outputs: values,
                    }),
                )
              }
              onInvalid={(name, source) =>
                studio.draft(
                  node.id,
                  "scope-output." + child.id + "." + name,
                  source,
                )
              }
            />
          </section>
        ))}
      <section className="mt-4">
        <h4 className="mb-2 text-[11px] text-muted">附加输出</h4>
        <OutputList
          values={node.output_bindings}
          symbols={[...symbols.values, ...results]}
          onChange={(values, name) =>
            studio.draft(node.id, "output." + name, null, (file) =>
              updateNode(file, node.id, (current) => ({
                ...current,
                output_bindings: values,
              })),
            )
          }
          onInvalid={(name, source) =>
            studio.draft(node.id, "output." + name, source)
          }
        />
      </section>
    </details>
  );
}
function OutputList({
  values,
  symbols,
  onChange,
  onInvalid,
}: {
  readonly values: Readonly<Record<string, Expr>>;
  readonly symbols: readonly SymbolValue[];
  readonly onChange: (
    values: Readonly<Record<string, Expr>>,
    name: string,
  ) => void;
  readonly onInvalid: (name: string, source: string) => void;
}) {
  return (
    <div className="space-y-3">
      {Object.entries(values).map(([name, value]) => {
        const type = inferExpression(value, symbols) ?? {
          type: "text" as const,
        };
        return (
          <div key={name} className="space-y-2">
            <div className="flex gap-1">
              <Input
                aria-label="输出名称"
                className="min-w-0 flex-1"
                defaultValue={name}
                onBlur={(event) => {
                  const next = event.target.value;
                  if (!next || next === name || values[next]) return;
                  const updated = { ...values, [next]: value };
                  delete updated[name];
                  onChange(updated, name);
                }}
              />
              <TypeSelect
                value={type}
                onChange={(type) =>
                  onChange(
                    { ...values, [name]: literal(type, defaultValue(type)) },
                    name,
                  )
                }
              />
              <Button
                variant="ghost"
                aria-label={"删除输出 " + name}
                className="px-1"
                onClick={() => {
                  const updated = { ...values };
                  delete updated[name];
                  onChange(updated, name);
                }}
              >
                <Trash2 size={12} />
              </Button>
            </div>
            <ValueField
              value={value}
              type={type}
              symbols={symbols}
              onChange={(value) => onChange({ ...values, [name]: value }, name)}
              onInvalid={(source) => onInvalid(name, source)}
            />
          </div>
        );
      })}
      <Button
        variant="ghost"
        className="h-6 px-0 text-[11px] text-muted"
        onClick={() => {
          let index = 1;
          while (values["output" + index]) index++;
          onChange(
            {
              ...values,
              ["output" + index]: literal(
                { type: "text" },
                { type: "text", value: "" },
              ),
            },
            "output" + index,
          );
        }}
      >
        <Plus size={12} />
        添加输出
      </Button>
    </div>
  );
}
