import { Collapse } from "../../ui";
import { FormulaEditor } from "./FormulaEditor";
import { useState } from "react";
import { Button, Dialog } from "../../ui";
import {
  compileExpressionSource,
  formatExpression,
  typeLabel,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";

/** 源码是编辑投影，应用后仍保存类型化 Expr，运行时无需解释文本。 */
export function ExpressionDialog({
  value,
  type,
  symbols,
  onApply,
  onClose,
}: {
  readonly value: Expr;
  readonly type: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly onApply: (value: Expr) => void;
  readonly onClose: () => void;
}) {
  const [source, setSource] = useState(() => formatExpression(value));
  const [error, setError] = useState("");
  return (
    <Dialog title="编辑表达式" wide onClose={onClose}>
      <p className="mb-3 text-xs leading-6 text-muted">
        使用计算式或函数得到{typeLabel(type)}。例如 line + 1、line &lt;
        3、concat("第", to_text(line))。
      </p>
      <FormulaEditor
        source={source}
        label="表达式"
        symbols={symbols}
        compile={(text) => {
          compileExpressionSource(text, symbols, type);
          return text;
        }}
        onChange={(text) => {
          setSource(text);
          setError("");
        }}
        onInvalid={(text) => {
          setSource(text);
          setError("请先修正表达式错误");
        }}
      />
      {symbols.length > 0 && (
        <Collapse className="mt-3 text-xs" title={<>可用的变量与步骤结果</>}>
          <div className="mt-2 flex flex-wrap gap-2">
            {symbols.map((symbol, index) => (
              <Button
                key={index}
                variant="ghost"
                className="h-auto bg-subtle text-left"
                title={symbol.label}
                onClick={() => {
                  setSource(
                    (current) => current + formatExpression(symbol.expression),
                  );
                  setError("");
                }}
              >
                {symbol.label} ·{" "}
                <code>{formatExpression(symbol.expression)}</code>
              </Button>
            ))}
          </div>
        </Collapse>
      )}
      <div className="mt-5 flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          取消
        </Button>
        <Button
          variant="primary"
          disabled={Boolean(error)}
          onClick={() => {
            try {
              onApply(compileExpressionSource(source, symbols, type));
            } catch (failure) {
              setError(
                failure instanceof Error ? failure.message : String(failure),
              );
            }
          }}
        >
          应用表达式
        </Button>
      </div>
    </Dialog>
  );
}
