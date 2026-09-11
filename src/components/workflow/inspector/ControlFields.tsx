import { useStore } from "zustand";
import { Plus, ArrowUpRight } from "lucide-react";
import {
  availableSymbols,
  defaultValue,
  literal,
  studio,
  updateNode,
  type Action,
  type EditorTab,
  type Expr,
  type ValueType,
  type WorkflowNode,
  type WorkflowFile,
} from "../../../features/workflow";
import { Button, FormField, Input, Select } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { TypeSelect } from "../value-editor/TypeSelect";
import { CallWorkflowFields } from "./CallWorkflowFields";
import { SwitchFields } from "./SwitchFields";
import { useDocuments } from "./useDocuments";

export function ControlFields({
  node,
  tab,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
}) {
  const state = useStore(studio.store);
  const documents = useDocuments();
  const symbols = availableSymbols(tab.file, tab.scope, node.id, documents);
  const action = node.action;
  const change = (next: Action) =>
    studio.changeNode(node.id, (current) => ({ ...current, action: next }));
  const valueField = (
    label: string,
    field: string,
    value: Expr,
    type: ValueType,
    update: (value: Expr) => Action,
  ) => (
    <FormField label={label}>
      <ValueField
        value={value}
        pending={tab.file.editor.drafts[node.id + ":" + field]}
        type={type}
        symbols={symbols.values}
        onChange={(value) =>
          studio.draft(node.id, field, null, (file) =>
            updateNode(file, node.id, (current) => ({
              ...current,
              action: update(value),
            })),
          )
        }
        onInvalid={(source) => studio.draft(node.id, field, source)}
      />
    </FormField>
  );
  switch (action.kind) {
    case "let":
      return (
        <div className="space-y-3">
          <div className="flex gap-2">
            <Input
              aria-label="变量名称"
              className="w-full"
              value={action.name}
              onChange={(event) =>
                change({ ...action, name: event.target.value })
              }
            />
            <TypeSelect
              value={action.value_type}
              onChange={(value_type) =>
                change({
                  ...action,
                  value_type,
                  value: literal(value_type, defaultValue(value_type)),
                })
              }
            />
          </div>
          {valueField(
            "初始值",
            "value",
            action.value,
            action.value_type,
            (value) => ({ ...action, value }),
          )}
        </div>
      );
    case "assign":
      return (
        <div className="space-y-3">
          {action.assignments.map((assignment, index) => (
            <div key={index} className="space-y-2">
              <Select
                aria-label="赋值目标"
                value={assignment.name}
                onValueChange={(selected) =>
                  change({
                    ...action,
                    assignments: action.assignments.map((item, position) =>
                      position === index ? { ...item, name: selected } : item,
                    ),
                  })
                }
                options={[
                  { value: "", label: "选择变量" },
                  ...symbols.values.flatMap((item) =>
                    item.expression.kind === "variable"
                      ? [{ value: item.expression.name, label: item.label }]
                      : [],
                  ),
                ]}
              />
              {valueField(
                "赋值为",
                "assignment." + index,
                assignment.value,
                symbols.values.find(
                  (item) =>
                    item.expression.kind === "variable" &&
                    item.expression.name === assignment.name,
                )?.type ?? { type: "int" },
                (value) => ({
                  ...action,
                  assignments: action.assignments.map((item, position) =>
                    position === index ? { ...item, value } : item,
                  ),
                }),
              )}
            </div>
          ))}
        </div>
      );
    case "if":
    case "while":
      return (
        <div className="space-y-4">
          {valueField(
            "条件",
            "condition",
            action.condition,
            { type: "bool" },
            (condition) => ({ ...action, condition }),
          )}
          {action.kind === "while" && (
            <FormField label="最多执行">
              <Input
                aria-label="最大循环次数"
                className="w-20"
                inputMode="numeric"
                value={action.max_iterations ?? ""}
                placeholder="默认"
                onChange={(event) =>
                  change({
                    ...action,
                    max_iterations: event.target.value
                      ? Math.max(1, Number(event.target.value))
                      : null,
                  })
                }
              />
              <span className="ml-2 text-xs text-muted">轮</span>
            </FormField>
          )}
          <p className="text-[11px] leading-5 text-muted">
            滚轮放大或双击容器，进入内部搭建步骤。
          </p>
        </div>
      );
    case "for_each":
      return (
        <div className="space-y-3">
          {valueField(
            "遍历",
            "items",
            action.items,
            action.items.kind === "list"
              ? { type: "list", of: action.items.item_type }
              : action.items.kind === "literal"
                ? action.items.value_type
                : { type: "list", of: { type: "text" } },
            (items) => ({ ...action, items }),
          )}
          <div className="flex items-center gap-2 text-xs text-muted">
            <span>每项</span>
            <Input
              aria-label="循环元素名称"
              className="w-24"
              value={action.item}
              onChange={(event) =>
                change({ ...action, item: event.target.value })
              }
            />
            <span>索引</span>
            <Input
              aria-label="循环索引名称"
              className="w-20"
              value={action.index}
              onChange={(event) =>
                change({ ...action, index: event.target.value })
              }
            />
          </div>
          <p className="text-[11px] leading-5 text-muted">
            在循环内部引用当前项；每轮的局部变量独立。
          </p>
        </div>
      );
    case "wait":
      return valueField(
        "等待时长",
        "milliseconds",
        action.milliseconds,
        { type: "int" },
        (milliseconds) => ({ ...action, milliseconds }),
      );
    case "fail":
      return (
        <FormField label="错误代码">
          <Input
            aria-label="错误代码"
            className="w-full"
            value={action.code}
            onChange={(event) =>
              change({ ...action, code: event.target.value })
            }
          />
        </FormField>
      );
    case "release":
      return (
        <FormField label="关闭资源">
          <Select
            aria-label="释放资源"
            className="max-w-full"
            value={action.resource}
            onValueChange={(value) => change({ ...action, resource: value })}
            options={[
              { value: "", label: "选择资源" },
              ...symbols.resources.map((item) => ({
                value: item.name,
                label: item.name,
              })),
            ]}
          />
        </FormField>
      );
    case "call_workflow":
      return <CallWorkflowFields node={node} tab={tab} action={action} />;
    case "return":
      return (
        <div className="space-y-3">
          {Object.entries(tab.file.definition.outputs).map(([key, type]) => (
            <div key={key}>
              {valueField(
                key,
                "return." + key,
                action.values[key] ?? literal(type, defaultValue(type)),
                type,
                (value) => ({
                  ...action,
                  values: { ...action.values, [key]: value },
                }),
              )}
            </div>
          ))}
          {!Object.keys(tab.file.definition.outputs).length && (
            <p className="text-xs text-muted">
              流程尚未声明输出。可在“输入输出”中添加。
            </p>
          )}
        </div>
      );
    case "switch":
      return <SwitchFields node={node} tab={tab} action={action} />;
    case "block":
      return (
        <p className="text-xs leading-5 text-muted">
          将相关步骤放在同一个容器中。放大容器进入内部编辑。
        </p>
      );
    case "break":
      return (
        <p className="text-xs leading-5 text-muted">
          结束当前循环，继续循环后的步骤。
        </p>
      );
    case "continue":
      return (
        <p className="text-xs leading-5 text-muted">
          跳过当前轮剩余步骤，进入下一轮。
        </p>
      );
    case "call":
      return <p className="text-xs text-muted">内部子流程：{action.subflow}</p>;
    case "task":
      return null;
  }
}
