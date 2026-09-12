import { Plus, Trash2 } from "lucide-react";
import {
  availableSymbols,
  addSwitchCase,
  removeSwitchCase,
  defaultValue,
  inferExpression,
  literal,
  studio,
  updateNode,
  type Action,
  type EditorTab,
  type WorkflowNode,
} from "../../../features/workflow";
import { Button, FormField, Select } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { LiteralInput } from "../value-editor/LiteralInput";
import { useDocuments } from "./useDocuments";

export function SwitchFields({
  node,
  tab,
  action,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
  readonly action: Extract<Action, { kind: "switch" }>;
}) {
  const symbols = availableSymbols(
    tab.file,
    tab.scope,
    node.id,
    useDocuments(),
  );
  const type = inferExpression(action.selector, symbols.values) ?? {
    type: "text",
  };
  const change = (next: typeof action, field: string) =>
    studio.draft(node.id, field, null, (file) =>
      updateNode(file, node.id, (current) => ({ ...current, action: next })),
    );
  const remove = (id: string) =>
    studio.edit((file) => removeSwitchCase(file, node.id, id));
  return (
    <div className="space-y-3">
      <FormField label="判断类型">
        <Select
          aria-label="分支类型"
          value={type.type}
          onValueChange={(selected) => {
            if (
              selected === "text" ||
              selected === "int" ||
              selected === "bool"
            ) {
              const nextType = { type: selected } as const;
              change(
                {
                  ...action,
                  selector: literal(nextType, defaultValue(nextType)),
                  cases: action.cases.map((item) => ({
                    ...item,
                    value: defaultValue(nextType),
                  })),
                },
                "selector",
              );
            }
          }}
          options={[
            { value: "text", label: "文本" },
            { value: "int", label: "整数" },
            { value: "bool", label: "布尔" },
          ]}
        />
      </FormField>
      <FormField label="选择值">
        <ValueField
          value={action.selector}
          type={type}
          symbols={symbols.values}
          onChange={(selector) => change({ ...action, selector }, "selector")}
          onInvalid={(source) => studio.draft(node.id, "selector", source)}
        />
      </FormField>
      {action.cases.map((item, index) => (
        <FormField key={item.scope} label={"分支 " + (index + 1)}>
          <LiteralInput
            value={item.value}
            type={type}
            pending={tab.file.editor.drafts[node.id + ":case." + item.scope]}
            onChange={(value) =>
              change(
                {
                  ...action,
                  cases: action.cases.map((entry) =>
                    entry.scope === item.scope ? { ...entry, value } : entry,
                  ),
                },
                "case." + item.scope,
              )
            }
            onInvalid={(source) =>
              studio.draft(node.id, "case." + item.scope, source)
            }
          />
          <Button
            variant="ghost"
            aria-label={"删除分支 " + (index + 1)}
            className="ml-auto px-1"
            onClick={() => remove(item.scope)}
          >
            <Trash2 size={12} />
          </Button>
        </FormField>
      ))}
      <Button
        variant="ghost"
        className="h-6 px-0 text-[11px]"
        onClick={() =>
          studio.edit((file) => {
            const value =
              type.type === "text"
                ? {
                    type: "text" as const,
                    value: "选项 " + (action.cases.length + 1),
                  }
                : type.type === "int"
                  ? {
                      type: "int" as const,
                      value: String(action.cases.length + 1),
                    }
                  : defaultValue(type);
            return addSwitchCase(file, node.id, value);
          })
        }
      >
        <Plus size={12} />
        添加分支
      </Button>
      <p className="text-[11px] text-muted">
        没有匹配值时执行默认分支。放大容器编辑各分支。
      </p>
    </div>
  );
}
