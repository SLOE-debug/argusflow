import type { BinaryOp } from "../../model/contracts";
/** 仅接受运行时已有的纯运算，符号用于解析与显示两端。 */
export const OPERATORS = {
  add: "+",
  subtract: "-",
  multiply: "*",
  divide: "/",
  remainder: "%",
  equal: "==",
  not_equal: "!=",
  less: "<",
  less_equal: "<=",
  greater: ">",
  greater_equal: ">=",
  and: "&&",
  or: "||",
} as const satisfies Readonly<Record<BinaryOp, string>>;
export function binaryOperator(symbol: string): BinaryOp {
  symbol = symbol === "===" ? "==" : symbol === "!==" ? "!=" : symbol;
  const entry = Object.entries(OPERATORS).find(([, value]) => value === symbol);
  if (!entry) throw new Error("不支持运算符：" + symbol);
  return entry[0] as BinaryOp;
}
