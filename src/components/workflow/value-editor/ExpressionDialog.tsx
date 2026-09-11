import { useState } from "react";
import { Button, Dialog, FormField, Select, Textarea } from "../../ui";
import {
  BINARY_OPS,
  defaultValue,
  inferExpression,
  literal,
  parseExpression,
  type BinaryOp,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";
import { TypeSelect } from "./TypeSelect";
import { ValueField } from "./ValueField";

const OP_NAMES: Readonly<Record<BinaryOp, string>> = {
  add: "加",
  subtract: "减",
  multiply: "乘",
  divide: "除",
  remainder: "余数",
  equal: "等于",
  not_equal: "不等于",
  less: "小于",
  less_equal: "小于或等于",
  greater: "大于",
  greater_equal: "大于或等于",
  and: "并且",
  or: "或者",
};
/** 常见运算直接组装类型化表达式；高级结构仍可显式编辑。 */
export function ExpressionDialog({
  value,
  type,
  symbols,
  onApply,
  onClose,
  depth,
}: {
  readonly value: Expr;
  readonly type: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly onApply: (value: Expr) => void;
  readonly onClose: () => void;
  readonly depth: number;
}) {
  const initial: Expr =
    value.kind === "literal" ||
    ["variable", "input", "node_output"].includes(value.kind)
      ? {
          kind: "binary",
          op: type.type === "bool" ? "equal" : "add",
          left:
            value.kind === "literal" && type.type === "bool"
              ? literal({ type: "int" }, { type: "int", value: "0" })
              : value,
          right: literal(
            type.type === "bool" ? { type: "int" } : type,
            defaultValue(type.type === "bool" ? { type: "int" } : type),
          ),
        }
      : value;
  const [draft, setDraft] = useState<Expr>(initial);
  const [advanced, setAdvanced] = useState(
    !["binary", "not"].includes(initial.kind),
  );
  const [source, setSource] = useState(JSON.stringify(initial, null, 2));
  const [error, setError] = useState("");
  const operandType =
    draft.kind === "binary"
      ? (inferExpression(draft.left, symbols) ?? { type: "int" as const })
      : { type: "bool" as const };
  const edit = (next: Expr) => {
    setDraft(next);
    setSource(JSON.stringify(next, null, 2));
    setError("");
  };
  return (
    <Dialog title="配置表达式" wide onClose={onClose}>
      {!advanced && draft.kind === "binary" && (
        <div className="space-y-4">
          <div className="flex items-center gap-3 text-xs text-muted">
            <span>比较或计算</span>
            <TypeSelect
              value={operandType}
              onChange={(next) =>
                edit({
                  ...draft,
                  left: literal(next, defaultValue(next)),
                  right: literal(next, defaultValue(next)),
                })
              }
            />
          </div>
          <FormField label="左侧">
            <ValueField
              depth={depth + 1}
              value={draft.left}
              type={operandType}
              symbols={symbols}
              onChange={(left) => edit({ ...draft, left })}
              onInvalid={setError}
            />
          </FormField>
          <FormField label="运算">
            <Select
              aria-label="运算符"
              value={draft.op}
              onValueChange={(selected) => {
                const op = BINARY_OPS.find((item) => item === selected);
                if (op) edit({ ...draft, op });
              }}
              options={BINARY_OPS.map((op) => ({
                value: op,
                label: OP_NAMES[op],
              }))}
            />
          </FormField>
          <FormField label="右侧">
            <ValueField
              depth={depth + 1}
              value={draft.right}
              type={operandType}
              symbols={symbols}
              onChange={(right) => edit({ ...draft, right })}
              onInvalid={setError}
            />
          </FormField>
        </div>
      )}
      {!advanced && draft.kind === "not" && (
        <FormField label="取反">
          <ValueField
            depth={depth + 1}
            value={draft.value}
            type={{ type: "bool" }}
            symbols={symbols}
            onChange={(value) => edit({ ...draft, value })}
            onInvalid={setError}
          />
        </FormField>
      )}
      {advanced && (
        <>
          <p className="mb-3 text-xs text-muted">
            编辑结构化表达式，支持字段访问、索引、函数和组合条件。运行前会检查类型。
          </p>
          <Textarea
            aria-label="表达式 JSON"
            className="h-64 w-full font-mono"
            value={source}
            onChange={(event) => {
              setSource(event.target.value);
              setError("");
            }}
          />
        </>
      )}
      {error && (
        <p role="alert" className="mt-3 text-xs text-danger">
          {error}
        </p>
      )}
      <div className="mt-6 flex justify-between gap-2">
        <Button
          variant="ghost"
          onClick={() => setAdvanced(!advanced)}
          disabled={advanced && !["binary", "not"].includes(draft.kind)}
        >
          {advanced ? "返回可视化编辑" : "高级表达式"}
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            try {
              if (error) return;
              const next = advanced ? parseExpression(source) : draft;
              onApply(next);
            } catch (failure) {
              setError(String(failure));
            }
          }}
        >
          应用表达式
        </Button>
      </div>
    </Dialog>
  );
}
