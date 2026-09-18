import { useState } from "react";
import { LinkButton } from "../../ui";
import {
  literal,
  defaultValue,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";
import { readCompoundDraft } from "../../../features/workflow/values/drafts";
import { ValueField } from "./ValueField";
/** 列表每项可以是固定值或引用，不要求用户编辑表达式内部结构。 */
export function ListExpressionEditor({
  items,
  itemType,
  symbols,
  onChange,
  onInvalid,
  pending,
  depth,
}: {
  readonly items: readonly Expr[];
  readonly itemType: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly onChange: (items: readonly Expr[]) => void;
  readonly onInvalid?: (source: string) => void;
  readonly pending?: string;
  readonly depth: number;
}) {
  const [invalid, setInvalid] = useState(() => readCompoundDraft(pending));
  // 删除后索引会移动，重新挂载字段并从重排后的草稿恢复，避免复用上一项的输入状态。
  const [revision, setRevision] = useState(0);
  function commit(
    next: readonly Expr[],
    drafts: Readonly<Record<string, string>>,
  ) {
    setInvalid(drafts);
    onChange(next);
    if (Object.keys(drafts).length) onInvalid?.(JSON.stringify(drafts));
  }
  return (
    <div className="min-w-0 space-y-2">
      {items.map((item, index) => (
        <div
          key={revision + ":" + index}
          className="flex min-w-0 items-start gap-2"
        >
          <span className="w-4 shrink-0 pt-2 text-xs text-muted">
            {index + 1}
          </span>
          <div className="min-w-0 flex-1">
            <ValueField
              value={item}
              type={itemType}
              symbols={symbols}
              depth={depth + 1}
              pending={invalid[index]}
              onChange={(value) => {
                const drafts = { ...invalid };
                delete drafts[index];
                commit(
                  items.map((old, i) => (i === index ? value : old)),
                  drafts,
                );
              }}
              onInvalid={(source) => {
                const drafts = { ...invalid, [index]: source };
                setInvalid(drafts);
                onInvalid?.(JSON.stringify(drafts));
              }}
            />
          </div>
          <LinkButton
            aria-label={"删除第 " + (index + 1) + " 项"}
            onClick={() => {
              setRevision((value) => value + 1);
              commit(
                items.filter((_, i) => i !== index),
                Object.fromEntries(
                  Object.entries(invalid)
                    .filter(([k]) => Number(k) !== index)
                    .map(([k, v]) => [
                      Number(k) > index ? Number(k) - 1 : k,
                      v,
                    ]),
                ),
              );
            }}
          >
            删除
          </LinkButton>
        </div>
      ))}
      <LinkButton
        onClick={() =>
          commit([...items, literal(itemType, defaultValue(itemType))], invalid)
        }
      >
        添加一项
      </LinkButton>
    </div>
  );
}
