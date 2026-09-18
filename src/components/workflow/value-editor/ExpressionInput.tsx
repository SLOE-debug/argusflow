import {
  studio,
  compileExpressionSource,
  formatExpression,
  type Expr,
  type SymbolValue,
  type ValueType,
} from "../../../features/workflow";
import { FormulaEditor } from "./FormulaEditor";
/** 内联表达式与弹窗共用 Monaco 和同一个类型检查器。 */
export function ExpressionInput({
  value,
  type,
  symbols,
  pending,
  onChange,
  onInvalid,
}: {
  readonly value: Expr;
  readonly type: ValueType;
  readonly symbols: readonly SymbolValue[];
  readonly pending?: string;
  readonly onChange: (value: Expr) => void;
  readonly onInvalid?: (source: string) => void;
}) {
  return (
    <FormulaEditor
      readOnly={studio.readonly}
      source={pending ?? formatExpression(value)}
      symbols={symbols}
      compile={(source) => compileExpressionSource(source, symbols, type)}
      onChange={onChange}
      onInvalid={onInvalid}
    />
  );
}
