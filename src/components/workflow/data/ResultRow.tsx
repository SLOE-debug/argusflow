import { Trash2 } from "lucide-react";
import { useState } from "react";
import {
  defaultValue,
  literal,
  renameWorkflowPort,
  scopeById,
  setWorkflowResult,
  studio,
  updateWorkflowPort,
  type EditorTab,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";
import { Button, Collapse, FormField, Input, Select } from "../../ui";
import { TypeSelect } from "../value-editor/TypeSelect";
import { ValueField } from "../value-editor/ValueField";
import { LiteralInput } from "../value-editor/LiteralInput";

/** 结果只展示名称与来源；选择来源时自动同步完整类型。 */
export function ResultRow({
  tab,
  name,
  type,
  symbols,
  canComplete,
}: {
  readonly tab: EditorTab;
  readonly name: string;
  readonly type: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly canComplete: boolean;
}) {
  const root = tab.file.definition.root;
  const [manualOpen, setManualOpen] = useState(false);
  const value =
    scopeById(tab.file, root).outputs[name] ??
    literal(type, defaultValue(type));
  const key = JSON.stringify(value);
  const source = symbols.find(
    (item) => JSON.stringify(item.expression) === key,
  );
  const pending = tab.file.editor.drafts[root + ":output." + name];
  const isReference = ["node_output", "input", "variable", "result"].includes(
    value.kind,
  );
  const missing = isReference && !source;
  const update = (next: Expr) =>
    studio.edit((file) => setWorkflowResult(file, name, type, next));
  const invalid = (text: string) => studio.draft(root, "output." + name, text);
  return (
    <div className="space-y-3 rounded-lg border border-line bg-subtle/40 p-3">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <FormField label="结果名称" stacked>
            <Input
              aria-label="结果名称"
              className="w-full"
              defaultValue={name}
              onBlur={(event) =>
                studio.edit((file) =>
                  renameWorkflowPort(
                    file,
                    "outputs",
                    name,
                    event.target.value.trim(),
                  ),
                )
              }
            />
          </FormField>
        </div>
        <Button
          variant="ghost"
          aria-label={"删除 " + name}
          className="mt-6 px-2"
          onClick={() =>
            studio.edit((file) =>
              updateWorkflowPort(file, "outputs", name, null),
            )
          }
        >
          <Trash2 size={14} />
        </Button>
      </div>
      {canComplete && (
        <FormField
          label="显示什么内容"
          stacked
          error={missing ? "原来选择的内容已不可用，请重新选择。" : undefined}
        >
          <Select
            aria-label="显示什么内容"
            aria-invalid={missing}
            className="w-full"
            value={key}
            options={[
              ...(!source
                ? [
                    {
                      value: key,
                      label: missing
                        ? "请重新选择"
                        : value.kind === "literal"
                          ? "手动填写的内容"
                          : "已设置的计算结果",
                      disabled: true,
                    },
                  ]
                : []),
              ...symbols.map((item) => ({
                value: JSON.stringify(item.expression),
                label: item.label,
              })),
            ]}
            onValueChange={(selected) => {
              const next = symbols.find(
                (item) => JSON.stringify(item.expression) === selected,
              );
              if (next)
                studio.edit((file) =>
                  setWorkflowResult(file, name, next.type, next.expression),
                );
            }}
          />
        </FormField>
      )}
      {canComplete && value.kind === "literal" && (
        <FormField label="填写内容" stacked>
          <LiteralInput
            key={JSON.stringify(type)}
            value={value.value}
            type={type}
            pending={pending}
            onInvalid={invalid}
            onChange={(next) => update(literal(type, next))}
          />
        </FormField>
      )}
      {pending && (
        <p role="alert" className="text-sm text-danger">
          手动设置的内容尚未填写完整，请展开下方设置检查，或重新选择已有结果。
        </p>
      )}
      <Collapse
        title="手动设置"
        onToggle={(event) => setManualOpen(event.currentTarget.open)}
      >
        {manualOpen && (
          <div className="space-y-3 pt-2">
            <TypeSelect
              value={type}
              onChange={(next) =>
                studio.edit((file) =>
                  updateWorkflowPort(file, "outputs", name, next),
                )
              }
            />
            {canComplete && (
              <ValueField
                key={JSON.stringify(type)}
                value={value}
                type={type}
                symbols={symbols}
                pending={pending}
                onInvalid={invalid}
                onChange={update}
              />
            )}
          </div>
        )}
      </Collapse>
    </div>
  );
}
