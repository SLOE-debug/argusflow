import { useState } from "react";
import { Settings2 } from "lucide-react";
import { Select, Button, Dialog } from "../../ui";
import { TypeShape } from "./TypeShape";
import { typeLabel, type ValueType } from "../../../features/workflow";
const TYPES: readonly ValueType[] = [
  { type: "text" },
  { type: "int" },
  { type: "float" },
  { type: "bool" },
  { type: "list", of: { type: "text" } },
  { type: "list", of: { type: "int" } },
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
    <div className="inline-flex items-start gap-1">
      <Select
        aria-label="数据类型"
        value={key}
        onValueChange={(selected) => {
          const type = TYPES.find((item) => JSON.stringify(item) === selected);
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
          aria-label="配置类型结构"
          className="h-7 px-1"
          onClick={() => setOpen(true)}
        >
          <Settings2 size={12} />
        </Button>
      )}
      {open && (
        <Dialog title="配置数据类型" wide onClose={() => setOpen(false)}>
          <TypeShape value={value} onChange={onChange} />
          <div className="mt-4 flex justify-end">
            <Button variant="primary" onClick={() => setOpen(false)}>
              完成
            </Button>
          </div>
        </Dialog>
      )}
    </div>
  );
}
