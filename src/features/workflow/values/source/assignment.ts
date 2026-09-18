import { formatExpression } from "./format";
import { compileExpressionSource } from "./parse";
import type { SymbolValue } from "../symbols";
import type { Expr, ValueType } from "../../model/contracts";
/** 单条赋值是节点编辑语法，不向表达式执行器开放任意脚本或副作用。 */
export function compileAssignment(
  source: string,
  symbols: readonly SymbolValue[],
  type?: ValueType,
): { readonly name: string; readonly value: Expr } {
  const update = /^\s*([\p{L}_$][\p{L}\p{N}_$]*)\s*(\+\+|--)\s*;?\s*$/u.exec(
    source,
  );
  if (update) {
    if (type) throw new Error("初始化变量请使用赋值语句，例如 line = 0");
    const name = update[1];
    const symbol = symbols.find(
      (item) =>
        item.expression.kind === "variable" && item.expression.name === name,
    );
    if (!symbol) throw new Error("变量未定义：" + name);
    if (symbol.type.type !== "int" && symbol.type.type !== "float")
      throw new Error("自增或自减只适用于数字变量");
    return {
      name,
      value: compileExpressionSource(
        name + (update[2] === "++" ? " + 1" : " - 1"),
        symbols,
        symbol.type,
      ),
    };
  }
  const match = /^\s*([\p{L}_$][\p{L}\p{N}_$]*)\s*=(?!=)([\s\S]+)$/u.exec(
    source,
  );
  if (!match) throw new Error("请填写赋值或更新语句，例如 line = 0、line++");
  const name = match[1];
  if (
    ["true", "false", "null", "inputs", "steps", "result", "vars"].includes(
      name,
    )
  )
    throw new Error("此名称是保留字，请更换变量名");
  const expected =
    type ??
    symbols.find(
      (item) =>
        item.expression.kind === "variable" && item.expression.name === name,
    )?.type;
  if (!expected) throw new Error("变量未定义：" + name);
  return {
    name,
    value: compileExpressionSource(
      match[2].trim().replace(/;$/, ""),
      symbols,
      expected,
    ),
  };
}

/** 常见数字增减在重新打开节点时仍呈现为简洁更新语句。 */
export function formatAssignment(name: string, value: Expr): string {
  if (
    value.kind === "binary" &&
    (value.op === "add" || value.op === "subtract") &&
    value.left.kind === "variable" &&
    value.left.name === name &&
    value.right.kind === "literal"
  ) {
    const right = value.right.value;
    if (
      (right.type === "int" && right.value === "1") ||
      (right.type === "float" && right.value === 1)
    )
      return name + (value.op === "add" ? "++" : "--");
  }
  return name + " = " + formatExpression(value);
}
