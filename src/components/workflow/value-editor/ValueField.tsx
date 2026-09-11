import { useState } from "react";
import { Code2 } from "lucide-react";
import { Button, Select } from "../../ui";
import {
  defaultValue,
  expressionLabel,
  literal,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";
import { LiteralInput } from "./LiteralInput";
import { ExpressionDialog } from "./ExpressionDialog";

interface Props {
  readonly value: Expr;
  readonly type: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly onChange: (value: Expr) => void;
  readonly onInvalid?: (source: string) => void;
  readonly depth?: number;
  readonly pending?: string;
}
/** 三种取值方式共同使用类型和作用域候选，未选引用不能沿用旧固定值。 */
export function ValueField({
  value,
  type,
  symbols,
  onChange,
  onInvalid,
  depth = 0,
  pending,
}: Props) {
  const inferredMode =
    value.kind === "literal"
      ? "literal"
      : ["variable", "input", "node_output", "result"].includes(value.kind)
        ? "reference"
        : "expression";
  const [emptyReference, setEmptyReference] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const mode = emptyReference ? "reference" : inferredMode;
  const candidates = symbols.filter(
    (item) => JSON.stringify(item.type) === JSON.stringify(type),
  );
  return (
    <div className="flex min-w-0 items-start gap-1">
      <div className="min-w-0 flex-1">
        {mode === "literal" && value.kind === "literal" ? (
          <LiteralInput
            value={value.value}
            type={type}
            pending={pending}
            onChange={(next) => onChange(literal(type, next))}
            onInvalid={onInvalid}
          />
        ) : mode === "reference" ? (
          <Select
            aria-label="引用值"
            className="max-w-full text-structure"
            value={emptyReference ? "" : JSON.stringify(value)}
            onValueChange={(selected) => {
              const item = candidates.find(
                (item) => JSON.stringify(item.expression) === selected,
              );
              if (item) {
                setEmptyReference(false);
                onChange(item.expression);
              }
            }}
            options={[
              ...(emptyReference
                ? [{ value: "", label: "没有可用引用", disabled: true }]
                : !candidates.some(
                      (item) =>
                        JSON.stringify(item.expression) ===
                        JSON.stringify(value),
                    )
                  ? [
                      {
                        value: JSON.stringify(value),
                        label: expressionLabel(value),
                        disabled: true,
                      },
                    ]
                  : []),
              ...candidates.map((item) => ({
                value: JSON.stringify(item.expression),
                label: item.label,
              })),
            ]}
          />
        ) : (
          <Button
            variant="ghost"
            className="h-7 max-w-full justify-start truncate bg-subtle text-structure"
            onClick={() => setExpanded(true)}
          >
            <Code2 size={12} />
            <span className="truncate">{expressionLabel(value)}</span>
          </Button>
        )}
      </div>
      <Select
        aria-label="取值方式"
        className="w-16 shrink-0 px-1 text-[10px] text-muted"
        value={mode}
        onValueChange={(next) => {
          if (next === "literal") {
            setEmptyReference(false);
            onChange(literal(type, defaultValue(type)));
          }
          if (next === "reference") {
            if (candidates[0]) {
              setEmptyReference(false);
              onChange(candidates[0].expression);
            } else {
              setEmptyReference(true);
              onInvalid?.("请选择可见且类型一致的引用");
            }
          }
          if (next === "expression") setExpanded(true);
        }}
        options={[
          { value: "literal", label: "固定" },
          { value: "reference", label: "引用" },
          { value: "expression", label: "表达式", disabled: depth >= 8 },
        ]}
      />
      {expanded && (
        <ExpressionDialog
          value={value}
          type={type}
          symbols={symbols}
          depth={depth}
          onClose={() => setExpanded(false)}
          onApply={(next) => {
            setEmptyReference(false);
            onChange(next);
            setExpanded(false);
          }}
        />
      )}
    </div>
  );
}
