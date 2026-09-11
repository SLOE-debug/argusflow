import { useStore } from "zustand";
import { ArrowUpRight, ChevronDown, Plus } from "lucide-react";
import {
  availableSymbols,
  defaultValue,
  literal,
  PORT_LABELS,
  studio,
  taskSpec,
  updateNode,
  type Expr,
  type JsonValue,
  type WorkflowNode,
  type EditorTab,
  type ValueType,
} from "../../../features/workflow";
import { Button, FormField, Input, Select, Switch } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { useDocuments } from "./useDocuments";
import { useTaskInputs } from "./useTaskInputs";

export function TaskFields({
  node,
  tab,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
}) {
  const documents = useDocuments();
  const types = useTaskInputs(
    node.action.kind === "task" ? node.action.task : null,
  );
  if (node.action.kind !== "task") return null;
  const task = node.action.task,
    spec = taskSpec(task.type_id);
  if (!spec) return <p className="text-xs text-danger">当前节点未注册</p>;
  const symbols = availableSymbols(tab.file, tab.scope, node.id, documents);
  const config = (key: string, value: JsonValue) =>
    studio.draft(node.id, "config." + key, null, (file) =>
      updateNode(file, node.id, (current) =>
        current.action.kind === "task"
          ? {
              ...current,
              action: {
                ...current.action,
                task: {
                  ...current.action.task,
                  config: { ...current.action.task.config, [key]: value },
                },
              },
            }
          : current,
      ),
    );
  const input = (key: string, value: Expr) =>
    studio.draft(node.id, "input." + key, null, (file) =>
      updateNode(file, node.id, (current) =>
        current.action.kind === "task"
          ? {
              ...current,
              action: {
                ...current.action,
                task: {
                  ...current.action.task,
                  inputs: { ...current.action.task.inputs, [key]: value },
                },
              },
            }
          : current,
      ),
    );
  const resource = (key: string, value: string, output = false) =>
    studio.draft(
      node.id,
      (output ? "created." : "resource.") + key,
      null,
      (file) =>
        updateNode(file, node.id, (current) => {
          if (current.action.kind !== "task") return current;
          const field = output ? "resource_outputs" : "resources";
          return {
            ...current,
            action: {
              ...current.action,
              task: {
                ...current.action.task,
                [field]: { ...current.action.task[field], [key]: value },
              },
            },
          };
        }),
    );
  const isQuery = spec.config.some((field) => field.type === "aql");
  return (
    <div className="space-y-5">
      {Object.entries(spec.resources).map(([port, type]) => (
        <div
          key={port}
          className="flex flex-wrap items-center gap-2 text-xs text-muted"
        >
          <span>{isQuery ? "在" : (PORT_LABELS[port] ?? port)}</span>
          <Select
            aria-label={PORT_LABELS[port] ?? port}
            className="max-w-full"
            value={task.resources[port] ?? ""}
            onValueChange={(value) => resource(port, value)}
            options={[
              { value: "", label: "选择" + (PORT_LABELS[port] ?? port) },
              ...symbols.resources
                .filter((item) => item.type === type)
                .map((item) => ({ value: item.name, label: item.name })),
            ]}
          />
          {isQuery && <span>中查找</span>}
        </div>
      ))}
      {spec.config.map((field) =>
        field.type === "aql" ? (
          <section key={field.key}>
            <div className="mb-2 flex items-center justify-between">
              <h3 className="text-xs font-medium">目标查询</h3>
              <Button
                variant="ghost"
                className="h-6 px-1 text-[11px] text-muted"
                onClick={() => studio.panel("aql", node.id)}
              >
                编辑
                <ArrowUpRight size={12} />
              </Button>
            </div>
            <Button
              variant="ghost"
              className="h-auto min-h-12 w-full justify-start whitespace-pre-wrap break-all bg-subtle px-3 py-2 text-left font-mono font-normal text-accent"
              onClick={() => studio.panel("aql", node.id)}
            >
              {tab.file.editor.drafts[node.id + ":aql"] ??
                String(task.config.query || "添加目标查询…")}
            </Button>
            <p
              className={
                "mt-1.5 text-[10px] " +
                (tab.file.editor.drafts[node.id + ":aql"] !== undefined
                  ? "text-danger"
                  : "text-muted")
              }
            >
              {tab.file.editor.drafts[node.id + ":aql"] !== undefined
                ? "查询尚未通过检查"
                : "中文编辑 · 英文执行"}
            </p>
          </section>
        ) : field.type === "boolean" ? (
          <Switch
            key={field.key}
            label={field.label}
            checked={Boolean(task.config[field.key])}
            onCheckedChange={(value) => config(field.key, value)}
          />
        ) : (
          <FormField key={field.key} label={field.label}>
            <Input
              aria-label={field.label}
              className={
                field.type === "number" || field.type === "u64"
                  ? "w-24"
                  : "w-full"
              }
              placeholder={field.optional ? "不限" : ""}
              value={
                tab.file.editor.drafts[node.id + ":config." + field.key] ??
                String(task.config[field.key] ?? "")
              }
              onChange={(event) => {
                const source = event.target.value;
                if (field.type === "u64") {
                  if (
                    /^\d+$/.test(source) &&
                    BigInt(source) <= 18446744073709551615n
                  )
                    config(field.key, source);
                  else studio.draft(node.id, "config." + field.key, source);
                } else if (field.type === "number") {
                  if (source.trim() && Number.isSafeInteger(Number(source)))
                    config(field.key, Number(source));
                  else studio.draft(node.id, "config." + field.key, source);
                } else if (field.optional && !source) {
                  studio.changeNode(node.id, (current) => {
                    if (current.action.kind !== "task") return current;
                    const fields = { ...current.action.task.config };
                    delete fields[field.key];
                    return {
                      ...current,
                      action: {
                        ...current.action,
                        task: { ...current.action.task, config: fields },
                      },
                    };
                  });
                } else config(field.key, source);
              }}
            />
          </FormField>
        ),
      )}
      {Object.keys(task.inputs).length > 0 && (
        <section className="space-y-2">
          <h3 className="mb-3 text-xs font-medium">参数</h3>
          {Object.entries(task.inputs).map(([key, value]) => {
            const type: ValueType =
              types[key] ??
              (value.kind === "literal" ? value.value_type : { type: "text" });
            return (
              <FormField key={key} label={PORT_LABELS[key] ?? key}>
                <ValueField
                  value={value}
                  type={type}
                  pending={tab.file.editor.drafts[node.id + ":input." + key]}
                  symbols={symbols.values}
                  onChange={(value) => input(key, value)}
                  onInvalid={(source) =>
                    studio.draft(node.id, "input." + key, source)
                  }
                />
              </FormField>
            );
          })}
        </section>
      )}
      {isQuery && (
        <Button
          variant="ghost"
          className="h-6 px-0 text-[11px] text-muted"
          onClick={() => studio.panel("aql", node.id)}
        >
          <Plus size={12} />
          在查询编辑器中配置参数
        </Button>
      )}
      {!!Object.keys(spec.creates).length && (
        <details className="border-t border-line pt-3">
          <summary className="cursor-pointer list-none text-xs text-muted">
            <span className="inline-flex items-center gap-2">
              <ChevronDown size={12} />
              创建的资源
            </span>
          </summary>
          <div className="mt-3 space-y-2">
            {Object.keys(spec.creates).map((port) => (
              <FormField key={port} label={PORT_LABELS[port] ?? port}>
                <Input
                  aria-label={"资源名称 " + port}
                  className="w-full"
                  value={task.resource_outputs[port] ?? ""}
                  onChange={(event) => resource(port, event.target.value, true)}
                />
              </FormField>
            ))}
          </div>
        </details>
      )}
    </div>
  );
}
