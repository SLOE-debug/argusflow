import { Collapse } from "../../ui";
import { QueryFields } from "./QueryFields";
import { KeyChordField } from "./KeyChordField";
import {
  availableSymbols,
  PORT_LABELS,
  studio,
  taskSpec,
  updateNode,
  isTargetTask,
  targetResources,
  targetPlatform,
  TARGET_PLATFORMS,
  changeTargetPlatform,
  type Expr,
  type JsonValue,
  type WorkflowNode,
  type EditorTab,
  type ValueType,
} from "../../../features/workflow";
import { FormField, Input, Select, Switch } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { useDocuments } from "./useDocuments";
import { useTaskInputs } from "./useTaskInputs";
import { LaunchArguments } from "./LaunchArguments";

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
  const resource = (key: string, value: string) =>
    studio.draft(node.id, "resource." + key, null, (file) =>
      updateNode(file, node.id, (current) => {
        if (current.action.kind !== "task") return current;
        return {
          ...current,
          action: {
            ...current.action,
            task: {
              ...current.action.task,
              resources: { ...current.action.task.resources, [key]: value },
            },
          },
        };
      }),
    );
  const isQuery = spec.config.some((field) => field.type === "aql");
  return (
    <div className="space-y-5">
      {isQuery && (
        <FormField label="平台" compact>
          <Select
            aria-label="自动化平台"
            value={targetPlatform(task.config) ?? ""}
            options={TARGET_PLATFORMS.map((value) => ({
              value,
              label: value.toUpperCase(),
            }))}
            onValueChange={(value) => {
              const platform = TARGET_PLATFORMS.find((item) => item === value);
              if (!platform) return;
              studio.changeNode(node.id, (current) =>
                current.action.kind === "task"
                  ? {
                      ...current,
                      action: {
                        ...current.action,
                        task: changeTargetPlatform(
                          current.action.task,
                          platform,
                        ),
                      },
                    }
                  : current,
              );
            }}
          />
        </FormField>
      )}
      {Object.entries(
        isTargetTask(task.type_id) ? targetResources(task) : spec.resources,
      ).map(([port, type]) => (
        <div
          key={port}
          className="flex flex-wrap items-center gap-2 text-xs text-muted"
        >
          <span>{PORT_LABELS[port] ?? port}</span>
          <Select
            aria-label={PORT_LABELS[port] ?? port}
            className="max-w-full"
            value={task.resources[port] ?? ""}
            onValueChange={(value) => resource(port, value)}
            options={[
              { value: "", label: "选择" + (PORT_LABELS[port] ?? port) },
              ...symbols.resources
                .filter((item) => item.type === type)
                .map((item) => ({ value: item.name, label: item.label })),
            ]}
          />
        </div>
      ))}
      {spec.config.map((field) =>
        field.type === "platform" ? null : field.type === "select" ? (
          <FormField key={field.key} label={field.label}>
            <Select
              aria-label={field.label}
              value={String(task.config[field.key] ?? "")}
              options={field.options ?? []}
              onValueChange={(value) => config(field.key, value)}
            />
          </FormField>
        ) : field.type === "aql" ? (
          <QueryFields key={node.id} tab={tab} nodeId={node.id} />
        ) : task.type_id === "aql.press_keys" && field.key === "keys" ? (
          <FormField key={field.key} label={field.label}>
            <KeyChordField
              value={String(task.config[field.key] ?? "")}
              onChange={(value) => config(field.key, value)}
            />
          </FormField>
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
      {!isQuery && Object.keys(task.inputs).length > 0 && (
        <section className="space-y-2">
          <h3 className="mb-3 text-xs font-medium">参数</h3>
          {Object.entries(task.inputs).map(([key, value]) => {
            const type: ValueType =
              types[key] ??
              (value.kind === "literal" ? value.value_type : { type: "text" });
            if (task.type_id === "application.launch" && key === "arguments")
              return (
                <FormField key={key} label="启动参数" stacked>
                  <LaunchArguments
                    value={value}
                    symbols={symbols.values}
                    pending={tab.file.editor.drafts[node.id + ":input." + key]}
                    onChange={(next) => input(key, next)}
                    onInvalid={(source) =>
                      studio.draft(node.id, "input." + key, source)
                    }
                  />
                </FormField>
              );
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
      {!!Object.keys(spec.creates).length && (
        <Collapse
          className="border-t border-line pt-3"
          title="此节点创建的资源"
        >
          <div className="mt-3 space-y-2">
            <p className="text-xs leading-5 text-muted">
              后续步骤通过此节点的名称选择资源，无需另取别名。
            </p>
            {Object.keys(spec.creates).map((port) => (
              <p key={port} className="text-xs">
                {PORT_LABELS[port] ?? port}
              </p>
            ))}
          </div>
        </Collapse>
      )}
    </div>
  );
}
