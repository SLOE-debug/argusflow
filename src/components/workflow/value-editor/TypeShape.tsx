import { Plus, Trash2 } from "lucide-react";
import { Button, Input, Select } from "../../ui";
import type { ValueType } from "../../../features/workflow";
/** 嵌套类型定义使用有限深度表单，不要求手写 JSON。 */
export function TypeShape({
  value,
  onChange,
  depth = 0,
}: {
  readonly value: ValueType;
  readonly onChange: (value: ValueType) => void;
  readonly depth?: number;
}) {
  return (
    <div className="space-y-3">
      <Select
        aria-label="字段类型"
        value={value.type}
        onValueChange={(selected) => {
          switch (selected) {
            case "text":
            case "int":
            case "float":
            case "bool":
              onChange({ type: selected });
              break;
            case "list":
            case "optional":
              onChange({ type: selected, of: { type: "text" } });
              break;
            case "record":
              onChange({ type: "record", of: {} });
              break;
          }
        }}
        options={[
          { value: "text", label: "文本" },
          { value: "int", label: "整数" },
          { value: "float", label: "小数" },
          { value: "bool", label: "布尔" },
          ...(depth < 8
            ? [
                { value: "list", label: "列表" },
                { value: "record", label: "记录" },
                { value: "optional", label: "可选值" },
              ]
            : []),
        ]}
      />
      {(value.type === "list" || value.type === "optional") && (
        <div className="border-l border-line pl-4">
          <TypeShape
            value={value.of}
            depth={depth + 1}
            onChange={(of) => onChange({ ...value, of })}
          />
        </div>
      )}
      {value.type === "record" && (
        <div className="space-y-3 border-l border-line pl-4">
          {Object.entries(value.of).map(([name, type]) => (
            <div key={name} className="flex items-start gap-2">
              <Input
                aria-label="字段名称"
                className="w-28"
                defaultValue={name}
                onBlur={(event) => {
                  const next = event.target.value.trim();
                  if (!next || next === name || value.of[next]) return;
                  const of = { ...value.of, [next]: type };
                  delete of[name];
                  onChange({ ...value, of });
                }}
              />
              <TypeShape
                value={type}
                depth={depth + 1}
                onChange={(next) =>
                  onChange({ ...value, of: { ...value.of, [name]: next } })
                }
              />
              <Button
                variant="ghost"
                aria-label={"删除字段 " + name}
                onClick={() => {
                  const of = { ...value.of };
                  delete of[name];
                  onChange({ ...value, of });
                }}
              >
                <Trash2 size={12} />
              </Button>
            </div>
          ))}
          <Button
            variant="ghost"
            onClick={() => {
              let index = 1;
              while (value.of["field" + index]) index++;
              onChange({
                ...value,
                of: { ...value.of, ["field" + index]: { type: "text" } },
              });
            }}
          >
            <Plus size={12} />
            添加字段
          </Button>
        </div>
      )}
    </div>
  );
}
