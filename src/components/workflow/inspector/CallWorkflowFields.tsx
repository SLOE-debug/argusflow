import { useEffect } from "react";
import { useStore } from "zustand";
import { ArrowUpRight } from "lucide-react";
import {
  availableSymbols,
  defaultValue,
  literal,
  studio,
  updateNode,
  type Action,
  type EditorTab,
  type WorkflowNode,
} from "../../../features/workflow";
import { Button, FormField, Select } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { useDocuments } from "./useDocuments";

export function CallWorkflowFields({
  node,
  tab,
  action,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
  readonly action: Extract<Action, { kind: "call_workflow" }>;
}) {
  const documents = useDocuments();
  const summaries = useStore(studio.store, (state) => state.documents);
  const target = documents.find((item) => item.id === action.workflow);
  const symbols = availableSymbols(tab.file, tab.scope, node.id, documents);
  useEffect(() => {
    if (action.workflow)
      void studio.safely(() => studio.reference(action.workflow));
  }, [action.workflow]);
  const change = (next: typeof action, field: string) =>
    studio.draft(node.id, field, null, (file) =>
      updateNode(file, node.id, (current) => ({ ...current, action: next })),
    );
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Select
          aria-label="调用工作流"
          className="min-w-0 flex-1"
          value={action.workflow}
          onValueChange={(id) => {
            if (!id) {
              change(
                { ...action, workflow: "", inputs: {}, resources: {} },
                "workflow",
              );
              return;
            }
            void studio.safely(async () => {
              const loaded = await studio.reference(id);
              if (studio.active?.file.id !== tab.file.id) return;
              change(
                {
                  ...action,
                  workflow: id,
                  inputs: Object.fromEntries(
                    Object.entries(loaded.definition.inputs).map(
                      ([name, type]) => [
                        name,
                        literal(type, defaultValue(type)),
                      ],
                    ),
                  ),
                  resources: Object.fromEntries(
                    Object.keys(loaded.definition.resources).map((name) => [
                      name,
                      "",
                    ]),
                  ),
                },
                "workflow",
              );
            });
          }}
          options={[
            { value: "", label: "选择工作流" },
            ...summaries
              .filter((item) => item.id !== tab.file.id)
              .map((item) => ({ value: item.id, label: item.name })),
          ]}
        />
        <Button
          variant="ghost"
          aria-label="打开引用的工作流"
          disabled={!action.workflow}
          onClick={() => {
            void studio.safely(() => studio.open(action.workflow));
          }}
        >
          <ArrowUpRight size={14} />
        </Button>
      </div>
      <p className="text-[11px] leading-5 text-muted">
        独立变量，通过参数传入数据。下次运行使用最新保存版本。
      </p>
      {Object.entries(action.inputs).map(([name, value]) => (
        <FormField key={name} label={name}>
          <ValueField
            value={value}
            type={
              target?.definition.inputs[name] ??
              (value.kind === "literal" ? value.value_type : { type: "text" })
            }
            symbols={symbols.values}
            pending={tab.file.editor.drafts[node.id + ":input." + name]}
            onChange={(next) =>
              change(
                { ...action, inputs: { ...action.inputs, [name]: next } },
                "input." + name,
              )
            }
            onInvalid={(source) =>
              studio.draft(node.id, "input." + name, source)
            }
          />
        </FormField>
      ))}
      {Object.entries(action.resources).map(([name, value]) => (
        <FormField key={name} label={name}>
          <Select
            aria-label={name}
            value={value}
            onValueChange={(selected) =>
              change(
                {
                  ...action,
                  resources: {
                    ...action.resources,
                    [name]: selected,
                  },
                },
                "resource." + name,
              )
            }
            options={[
              { value: "", label: "选择资源" },
              ...symbols.resources
                .filter(
                  (item) => item.type === target?.definition.resources[name],
                )
                .map((item) => ({ value: item.name, label: item.name })),
            ]}
          />
        </FormField>
      ))}
    </div>
  );
}
