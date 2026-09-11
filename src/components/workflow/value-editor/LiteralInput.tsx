import { useState } from "react";
import { Input, Select, Button, Dialog, FormField } from "../../ui";
import {
  defaultValue,
  isValue,
  type Value,
  type ValueType,
} from "../../../features/workflow";
import { readCompoundDraft } from "../../../features/workflow/values/drafts";

/** 标量控件按内容定宽，复合值只在显式展开时占据编辑空间。 */
export function LiteralInput({
  value,
  type,
  onChange,
  onInvalid,
  pending,
}: {
  readonly value: Value;
  readonly type: ValueType;
  readonly onChange: (value: Value) => void;
  readonly onInvalid?: (source: string) => void;
  readonly pending?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [draft, setDraft] = useState<string | null>(pending ?? null);
  const [error, setError] = useState("");
  const [children, setChildren] = useState<Readonly<Record<string, string>>>(
    () => readCompoundDraft(pending),
  );
  const invalidChild = (name: string, source: string) => {
    const next = { ...children, [name]: source };
    setChildren(next);
    onInvalid?.(JSON.stringify(next));
  };
  const commitChild = (next: Value, name?: string) => {
    const remaining = { ...children };
    if (name !== undefined) delete remaining[name];
    setChildren(remaining);
    commit(next);
    if (Object.keys(remaining).length) onInvalid?.(JSON.stringify(remaining));
  };
  const invalid = (source: string, message: string) => {
    setDraft(source);
    setError(message);
    onInvalid?.(source);
  };
  const commit = (next: Value) => {
    setDraft(null);
    setError("");
    onChange(next);
  };
  if (value.type === "bool")
    return (
      <Select
        aria-label="布尔值"
        value={String(value.value)}
        onValueChange={(value) =>
          commit({ type: "bool", value: value === "true" })
        }
        options={[
          { value: "true", label: "真" },
          { value: "false", label: "假" },
        ]}
      />
    );
  if (value.type === "text" || value.type === "int" || value.type === "float")
    return (
      <div className="min-w-0">
        <Input
          aria-label="固定值"
          aria-invalid={Boolean(error)}
          className={value.type === "text" ? "w-full" : "w-24 tabular-nums"}
          value={draft ?? String(value.value)}
          inputMode={value.type === "text" ? "text" : "decimal"}
          onChange={(event) => {
            const source = event.target.value;
            if (value.type === "text") commit({ type: "text", value: source });
            else if (value.type === "int") {
              const next = { type: "int", value: source };
              if (isValue(next)) commit(next);
              else invalid(source, "请输入有效整数，且不超出支持范围");
            } else if (source.trim() && Number.isFinite(Number(source)))
              commit({ type: "float", value: Number(source) });
            else invalid(source, "请输入有限小数");
          }}
        />
        {error && <p className="mt-1 text-[10px] text-danger">{error}</p>}
      </div>
    );
  return (
    <>
      <Button
        variant="ghost"
        className="max-w-full truncate bg-subtle font-normal"
        onClick={() => setExpanded(true)}
      >
        {value.type === "list"
          ? value.value.length + " 项"
          : value.type === "record"
            ? Object.keys(value.value).length + " 个字段"
            : value.value === null
              ? "无值"
              : "已设置"}{" "}
        · 编辑
      </Button>
      {expanded && (
        <Dialog title="编辑结构化值" wide onClose={() => setExpanded(false)}>
          <div className="max-h-80 space-y-3 overflow-auto">
            {value.type === "list" && type.type === "list" && (
              <>
                {value.value.map((item, index) => (
                  <div key={index} className="flex items-center gap-3">
                    <span className="w-5 text-xs text-muted">{index + 1}</span>
                    <div className="min-w-0 flex-1">
                      <LiteralInput
                        value={item}
                        type={type.of}
                        pending={children[index]}
                        onChange={(next) =>
                          commitChild(
                            {
                              ...value,
                              value: value.value.map((entry, position) =>
                                position === index ? next : entry,
                              ),
                            },
                            String(index),
                          )
                        }
                        onInvalid={(source) =>
                          invalidChild(String(index), source)
                        }
                      />
                    </div>
                    <Button
                      variant="ghost"
                      aria-label={"删除第 " + (index + 1) + " 项"}
                      onClick={() => {
                        const remaining = Object.fromEntries(
                          Object.entries(children)
                            .filter(([key]) => Number(key) !== index)
                            .map(([key, source]) => [
                              Number(key) > index
                                ? String(Number(key) - 1)
                                : key,
                              source,
                            ]),
                        );
                        setChildren(remaining);
                        commit({
                          ...value,
                          value: value.value.filter(
                            (_, position) => position !== index,
                          ),
                        });
                        if (Object.keys(remaining).length)
                          onInvalid?.(JSON.stringify(remaining));
                      }}
                    >
                      删除
                    </Button>
                  </div>
                ))}
                <Button
                  onClick={() =>
                    commitChild({
                      ...value,
                      value: [...value.value, defaultValue(type.of)],
                    })
                  }
                >
                  添加一项
                </Button>
              </>
            )}
            {value.type === "record" &&
              type.type === "record" &&
              Object.entries(type.of).map(([name, fieldType]) => (
                <FormField key={name} label={name}>
                  <LiteralInput
                    value={value.value[name] ?? defaultValue(fieldType)}
                    type={fieldType}
                    pending={children[name]}
                    onChange={(next) =>
                      commitChild(
                        {
                          ...value,
                          value: { ...value.value, [name]: next },
                        },
                        name,
                      )
                    }
                    onInvalid={(source) => invalidChild(name, source)}
                  />
                </FormField>
              ))}
            {value.type === "optional" && type.type === "optional" && (
              <>
                <Select
                  aria-label="是否有值"
                  value={value.value === null ? "none" : "some"}
                  onValueChange={(selected) =>
                    commit({
                      ...value,
                      value: selected === "none" ? null : defaultValue(type.of),
                    })
                  }
                  options={[
                    { value: "none", label: "无值" },
                    { value: "some", label: "有值" },
                  ]}
                />
                {value.value !== null && (
                  <LiteralInput
                    value={value.value}
                    type={type.of}
                    pending={children.value}
                    onChange={(next) =>
                      commitChild({ ...value, value: next }, "value")
                    }
                    onInvalid={(source) => invalidChild("value", source)}
                  />
                )}
              </>
            )}
          </div>
          {error && <p className="mt-2 text-xs text-danger">{error}</p>}
          <div className="mt-4 flex justify-end gap-2">
            <Button
              onClick={() => {
                setChildren({});
                commit(defaultValue(type));
              }}
            >
              重置
            </Button>
            <Button variant="primary" onClick={() => setExpanded(false)}>
              完成
            </Button>
          </div>
        </Dialog>
      )}
    </>
  );
}
