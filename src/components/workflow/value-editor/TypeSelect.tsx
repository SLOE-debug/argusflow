import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { Select, Button } from "../../ui";
import { TypeShape } from "./TypeShape";
import { typeLabel, type ValueType } from "../../../features/workflow";
const TYPES: readonly ValueType[] = [
  { type: "text" },
  { type: "int" },
  { type: "float" },
  { type: "bool" },
  { type: "list", of: { type: "text" } },
  { type: "list", of: { type: "int" } },
  { type: "list", of: { type: "record", of: {} } },
  { type: "record", of: {} },
  { type: "optional", of: { type: "text" } },
];
export function TypeSelect({
  value,
  onChange,
}: {
  readonly value: ValueType;
  readonly onChange: (value: ValueType) => void;
}) {
  const key = JSON.stringify(value);
  const [open, setOpen] = useState(false);
  return (
    <div className="min-w-0 space-y-2">
      <div className="flex flex-wrap items-center gap-1">
        <Select
          aria-label="数据类型"
          value={key}
          onValueChange={(selected) => {
            const type = TYPES.find(
              (item) => JSON.stringify(item) === selected,
            );
            if (type) onChange(type);
          }}
          options={[
            ...(!TYPES.some((item) => JSON.stringify(item) === key)
              ? [{ value: key, label: typeLabel(value) }]
              : []),
            ...TYPES.map((item) => ({
              value: JSON.stringify(item),
              label: typeLabel(item),
            })),
          ]}
        />
        {["record", "list", "optional"].includes(value.type) && (
          <Button
            variant="ghost"
            aria-expanded={open}
            className="h-7 px-1 text-xs text-muted"
            onClick={() => setOpen(!open)}
          >
            <ChevronDown size={12} className={open ? "rotate-180" : ""} />
            {value.type === "record" ||
            (value.type === "list" && value.of.type === "record")
              ? "字段设置"
              : "内容类型"}
          </Button>
        )}
      </div>
      {open && ["record", "list", "optional"].includes(value.type) && (
        <div className="rounded-md border border-line bg-surface p-3">
          <TypeShape value={value} onChange={onChange} showType={false} />
        </div>
      )}
    </div>
  );
}
