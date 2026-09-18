import { Plus, Trash2 } from "lucide-react";
import { Button, Collapse, Input, Select } from "../../ui";
import { studio, type EditorTab } from "../../../features/workflow";

/** 管理其他流程调用时传入的资源。 */
export function ResourcePorts({ tab }: { readonly tab: EditorTab }) {
  const resources = tab.file.definition.resources;
  return (
    <Collapse
      className="mt-5 border-t border-line pt-3"
      title={<>外部传入的资源 · {Object.keys(resources).length}</>}
    >
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
          添加外部资源
        </Button>
        <p className="text-[10px] leading-5 text-muted">
          由调用此流程的其他流程传入，例如已打开的浏览器页面或窗口。独立运行时，请在流程中创建资源。
        </p>
      </div>
    </Collapse>
  );
}
