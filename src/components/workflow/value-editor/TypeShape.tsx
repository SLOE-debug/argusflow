import { Plus, Trash2 } from "lucide-react";
import { LinkButton, Button, Input, Select } from "../../ui";
import type { ValueType } from "../../../features/workflow";
/** 嵌套类型定义使用有限深度表单，不要求手写 JSON。 */
export function TypeShape({
  value,
  onChange,
  depth = 0,
  showType = true,
}: {
  readonly value: ValueType;
  readonly onChange: (value: ValueType) => void;
  readonly depth?: number;
  /** 外层已有类型选择时，仅展示内容和字段配置。 */
  readonly showType?: boolean;
}) {
  return (
    <div className="space-y-3">
      {showType && (
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
      )}
      {(value.type === "list" || value.type === "optional") && (
        <div className="space-y-2">
          <p className="text-xs text-muted">
            {value.type === "list" ? "每项的数据类型" : "有值时的数据类型"}
          </p>
          <TypeShape
            value={value.of}
            depth={depth + 1}
            onChange={(of) => onChange({ ...value, of })}
          />
        </div>
      )}
      {value.type === "record" && (
        <div className="space-y-3">
          <p className="text-xs text-muted">字段名称与类型</p>
          {!Object.keys(value.of).length && (
            <p className="text-xs text-muted">
              还没有字段，添加需要保存的数据，例如名称、位置。
            </p>
          )}
          {Object.entries(value.of).map(([name, type]) => (
            <div
              key={name}
              className="flex flex-wrap items-start gap-2 border-l border-line pl-3"
            >
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
          <LinkButton
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
          </LinkButton>
        </div>
      )}
    </div>
  );
}
