import { Plus, Trash2 } from "lucide-react";
import {
  nextPortName,
  renameWorkflowPort,
  studio,
  updateWorkflowPort,
  type EditorTab,
  type ValueType,
} from "../../../features/workflow";
import { Button, Collapse, FormField, Input, Select } from "../../ui";
import { TypeSelect } from "../value-editor/TypeSelect";
import { ResourcePorts } from "./ResourcePorts";

/** 常用填写方式以用途说明，复杂结构仅留在更多设置中。 */
const INPUT_KINDS = [
  { value: "text", label: "文字，如文件位置" },
  { value: "int", label: "整数，如重复次数" },
  { value: "float", label: "数字，可带小数" },
  { value: "bool", label: "是或否" },
] as const;

/** 让创建者按填写表单的方式定义运行前需要的内容。 */
export function WorkflowInputs({ tab }: { readonly tab: EditorTab }) {
  const inputs = tab.file.definition.inputs;
  const update = (name: string, type: ValueType | null) =>
    studio.edit((file) => updateWorkflowPort(file, "inputs", name, type));
  return (
    <section className="min-w-0 p-4">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold">开始前填写</h3>
        <Button
          variant="ghost"
          onClick={() =>
            studio.edit((file) =>
              updateWorkflowPort(
                file,
                "inputs",
                nextPortName(file.definition.inputs, "填写项"),
                { type: "text" },
              ),
            )
          }
        >
          <Plus size={14} />
          添加填写项
        </Button>
      </div>
      <p className="mb-4 text-sm leading-6 text-muted">
        每次运行前需要填写的内容，例如文件保存位置、重复次数。
      </p>
      <div className="space-y-3">
        {Object.entries(inputs).map(([name, type]) => (
          <div
            key={name}
            className="space-y-3 rounded-lg border border-line bg-subtle/40 p-3"
          >
            <div className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <FormField label="填写项名称" stacked>
                  <Input
                    aria-label="填写项名称"
                    className="w-full"
                    defaultValue={name}
                    placeholder="例如：文件保存位置"
                    onBlur={(event) =>
                      studio.edit((file) =>
                        renameWorkflowPort(
                          file,
                          "inputs",
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
                onClick={() => update(name, null)}
              >
                <Trash2 size={14} />
              </Button>
            </div>
            <FormField label="需要填写什么" stacked>
              <Select
                aria-label="需要填写什么"
                className="w-full"
                value={type.type}
                options={[
                  ...INPUT_KINDS,
                  ...(!INPUT_KINDS.some((item) => item.value === type.type)
                    ? [
                        {
                          value: type.type,
                          label: "多项内容（已设置）",
                          disabled: true,
                        },
                      ]
                    : []),
                ]}
                onValueChange={(selected) => {
                  const option = INPUT_KINDS.find(
                    (item) => item.value === selected,
                  );
                  if (option) update(name, { type: option.value });
                }}
              />
            </FormField>
            <Collapse title="更多设置">
              <div className="pt-2">
                <TypeSelect
                  value={type}
                  onChange={(next) => update(name, next)}
                />
              </div>
            </Collapse>
          </div>
        ))}
        {!Object.keys(inputs).length && (
          <p className="text-sm text-muted">
            没有需要提前填写的内容时，无需添加填写项。
          </p>
        )}
      </div>
      <ResourcePorts tab={tab} />
    </section>
  );
}
