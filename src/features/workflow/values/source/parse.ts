import jsep from "jsep";
import objectPlugin from "@jsep-plugin/object";
import type { Expr, ValueType } from "../../model/contracts";
import type { SymbolValue } from "../symbols";
import { convertExpression } from "./convert";
import { expressionType, sameType } from "./types";
jsep.plugins.register(objectPlugin);

/** 解析前约束输入规模，避免第三方递归解析器消耗无界栈或时间。 */
function budget(source: string): void {
  if (!source.trim() || source.length > 16384)
    throw new Error("表达式应为 1–16384 个字符");
  let quote = "",
    escaped = false,
    depth = 0,
    tokens = 0;
  for (const char of source) {
    if (quote) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === quote) quote = "";
    } else if (char === '"' || char === "'") quote = char;
    else {
      if ("([{".includes(char) && ++depth > 48)
        throw new Error("表达式嵌套不能超过 48 层");
      if (")]}".includes(char)) depth--;
      if (!/[\p{L}\p{N}_$\s.]/u.test(char) && ++tokens > 512)
        throw new Error("表达式过于复杂，请拆成多个节点");
    }
  }
}
/** 编辑时解析一次，保存现有 Expr；循环运行不再解析源码。 */
export function compileExpressionSource(
  source: string,
  symbols: readonly SymbolValue[],
  expected: ValueType,
): Expr {
  budget(source);
  let parsed: jsep.Expression;
  try {
    parsed = jsep(source);
  } catch (error) {
    const position =
      error instanceof Error &&
      "index" in error &&
      typeof error.index === "number"
        ? "，请检查第 " + (error.index + 1) + " 个字符附近"
        : "，请检查括号、引号和运算符";
    throw new Error("表达式未完成或语法有误" + position, { cause: error });
  }
  const expression = convertExpression(parsed, symbols, expected);
  const actual = expressionType(expression, symbols);
  if (!sameType(actual, expected))
    throw new Error(
      "结果类型为 " + actual.type + "，此处需要 " + expected.type,
    );
  return expression;
}
