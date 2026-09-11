import { Plus, Trash2 } from "lucide-react";
import {
  availableSymbols,
  defaultValue,
  literal,
  replaceScope,
  scopeById,
  studio,
  type EditorTab,
  type ValueType,
} from "../../../features/workflow";
import { Button, Input, Select } from "../../ui";
import { TypeSelect } from "../value-editor/TypeSelect";
import { ValueField } from "../value-editor/ValueField";
import { scopeCanComplete } from "../../../features/workflow/model/outputs";
import { RunOutputs } from "./RunOutputs";
import { useDocuments } from "../inspector/useDocuments";
/** 流程公开端口与根出口表达式集中管理，变量由画布声明。 */
export function DataPanel({ tab }: { readonly tab: EditorTab }) {
  const documents = useDocuments();
  const definition = tab.file.definition;
  const updateFields = (
    field: "inputs" | "outputs",
    name: string,
    type: ValueType | null,
  ) =>
    studio.edit((file) => {
      const fields = { ...file.definition[field] };
      if (type) fields[name] = type;
      else delete fields[name];
      let next = {
        ...file,
        definition: { ...file.definition, [field]: fields },
      };
      if (field === "outputs") {
        const root = scopeById(next, definition.root),
          outputs = { ...root.outputs };
        if (type) outputs[name] = literal(type, defaultValue(type));
        else delete outputs[name];
        next = replaceScope(next, { ...root, outputs });
      }
      return next;
    });
  const add = (field: "inputs" | "outputs") => {
    const prefix = field === "inputs" ? "input" : "output";
    let index = 1;
    while (definition[field][prefix + index]) index++;
    updateFields(field, prefix + index, { type: "text" });
  };
  const rename = (field: "inputs" | "outputs", old: string, name: string) => {
    if (!name.trim() || name === old || definition[field][name]) return;
    studio.edit((file) => {
      const fields = {
        ...file.definition[field],
        [name]: file.definition[field][old],
      };
      delete fields[old];
      let next = {
        ...file,
        definition: { ...file.definition, [field]: fields },
      };
      if (field === "outputs") {
        const root = scopeById(next, definition.root),
          outputs = { ...root.outputs, [name]: root.outputs[old] };
        delete outputs[old];
        next = replaceScope(next, { ...root, outputs });
      }
      return next;
    });
  };
  const symbols = availableSymbols(
    tab.file,
    definition.root,
    undefined,
    documents,
  );
  return (
    <fieldset
      disabled={studio.readonly}
      className="grid h-full grid-cols-2 divide-x divide-line overflow-auto"
    >
      {(["inputs", "outputs"] as const).map((field) => (
        <section key={field} className="min-w-0 p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-xs font-semibold">
              {field === "inputs" ? "流程输入" : "流程输出"}
            </h3>
            <Button
              variant="ghost"
              className="h-6 px-1"
              onClick={() => add(field)}
            >
              <Plus size={13} />
              添加
            </Button>
          </div>
          <div className="space-y-2">
            {Object.entries(definition[field]).map(([name, type]) => (
              <div key={name} className="space-y-1.5">
                <div className="flex min-w-0 items-center gap-2">
                  <Input
                    aria-label={field + " 名称"}
                    className="w-28"
                    defaultValue={name}
                    onBlur={(event) => rename(field, name, event.target.value)}
                  />
                  <TypeSelect
                    value={type}
                    onChange={(type) => updateFields(field, name, type)}
                  />
                  <Button
                    variant="ghost"
                    aria-label={"删除 " + name}
                    className="ml-auto h-7 px-1.5"
                    onClick={() => updateFields(field, name, null)}
                  >
                    <Trash2 size={12} />
                  </Button>
                </div>
                {field === "outputs" &&
                  scopeCanComplete(tab.file, definition.root) && (
                    <ValueField
                      value={
                        scopeById(tab.file, definition.root).outputs[name] ??
                        literal(type, defaultValue(type))
                      }
                      type={type}
                      symbols={symbols.values}
                      onInvalid={(source) =>
                        studio.draft(definition.root, "output." + name, source)
                      }
                      onChange={(value) =>
                        studio.draft(
                          definition.root,
                          "output." + name,
                          null,
                          (file) => {
                            const root = scopeById(file, definition.root);
                            return replaceScope(file, {
                              ...root,
                              outputs: { ...root.outputs, [name]: value },
                            });
                          },
                        )
                      }
                    />
                  )}
                {field === "outputs" &&
                  !scopeCanComplete(tab.file, definition.root) && (
                    <p className="text-[10px] text-muted">
                      在返回节点中提供此值。
                    </p>
                  )}
              </div>
            ))}
            {!Object.keys(definition[field]).length && (
              <p className="text-[11px] text-muted">
                {field === "inputs"
                  ? "添加运行时需要填写的参数。"
                  : "添加对调用方公开的返回值。"}
              </p>
            )}
          </div>
          {field === "inputs" && <ResourcePorts tab={tab} />}
          {field === "outputs" && <RunOutputs workflow={tab.file.id} />}
        </section>
      ))}
    </fieldset>
  );
}
function ResourcePorts({ tab }: { readonly tab: EditorTab }) {
  const resources = tab.file.definition.resources;
  return (
    <details className="mt-5 border-t border-line pt-3">
      <summary className="cursor-pointer text-xs text-muted">
        借用资源端口 · {Object.keys(resources).length}
      </summary>
      <div className="mt-3 space-y-2">
        {Object.entries(resources).map(([name, type]) => (
          <div key={name} className="flex gap-2">
            <Input
              aria-label="资源端口名"
              className="w-24"
              defaultValue={name}
              onBlur={(event) => {
                const next = event.target.value;
                if (next && next !== name && !resources[next])
                  studio.edit((file) => {
                    const fields = {
                      ...file.definition.resources,
                      [next]: type,
                    };
                    delete fields[name];
                    return {
                      ...file,
                      definition: { ...file.definition, resources: fields },
                    };
                  });
              }}
            />
            <Select
              aria-label="资源类型"
              className="min-w-0 flex-1"
              value={type}
              onValueChange={(selected) =>
                studio.edit((file) => ({
                  ...file,
                  definition: {
                    ...file.definition,
                    resources: {
                      ...file.definition.resources,
                      [name]: selected,
                    },
                  },
                }))
              }
              options={[
                "browser",
                "page",
                "query_source",
                "application",
                "window",
              ].map((kind) => ({ value: "automation." + kind, label: kind }))}
            />
            <Button
              variant="ghost"
              aria-label="删除资源端口"
              className="px-1"
              onClick={() =>
                studio.edit((file) => {
                  const fields = { ...file.definition.resources };
                  delete fields[name];
                  return {
                    ...file,
                    definition: { ...file.definition, resources: fields },
                  };
                })
              }
            >
              <Trash2 size={12} />
            </Button>
          </div>
        ))}
        <Button
          variant="ghost"
          className="h-6 px-0 text-[11px]"
          onClick={() => {
            let index = 1;
            while (resources["resource" + index]) index++;
            studio.edit((file) => ({
              ...file,
              definition: {
                ...file.definition,
                resources: {
                  ...file.definition.resources,
                  ["resource" + index]: "automation.page",
                },
              },
            }));
          }}
        >
          <Plus size={12} />
          添加借用资源
        </Button>
        <p className="text-[10px] leading-5 text-muted">
          借用端口用于被其他流程调用；独立运行时应在流程内部创建资源。
        </p>
      </div>
    </details>
  );
}
