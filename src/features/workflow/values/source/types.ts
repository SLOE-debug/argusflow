import type { Expr, ValueType } from "../../model/contracts";
import type { SymbolValue } from "../symbols";
/** 结构类型比较与字段顺序无关。 */
export function sameType(a: ValueType, b: ValueType): boolean {
  if (a.type !== b.type) return false;
  if (a.type === "list" || a.type === "optional")
    return (b.type === "list" || b.type === "optional") && sameType(a.of, b.of);
  if (a.type === "record")
    return (
      b.type === "record" &&
      Object.keys(a.of).length === Object.keys(b.of).length &&
      Object.entries(a.of).every(([k, t]) => b.of[k] && sameType(t, b.of[k]))
    );
  return true;
}
const bool: ValueType = { type: "bool" },
  text: ValueType = { type: "text" },
  int: ValueType = { type: "int" };
function requireType(actual: ValueType, expected: ValueType): void {
  if (!sameType(actual, expected))
    throw new Error("表达式类型不匹配：" + actual.type + " / " + expected.type);
}
/** 编辑时检查可见引用与纯表达式类型；运行前 Rust 编译器仍作最终校验。 */
export function expressionType(
  expr: Expr,
  symbols: readonly SymbolValue[],
  depth = 0,
): ValueType {
  if (depth > 48) throw new Error("表达式嵌套不能超过 48 层");
  const type = (value: Expr) => expressionType(value, symbols, depth + 1);
  switch (expr.kind) {
    case "literal":
      return expr.value_type;
    case "variable":
    case "input":
    case "node_output":
    case "result": {
      const symbol = symbols.find(({ expression: value }) => {
        if (expr.kind === "variable" || expr.kind === "input")
          return value.kind === expr.kind && value.name === expr.name;
        if (expr.kind === "node_output")
          return (
            value.kind === "node_output" &&
            value.node === expr.node &&
            value.output === expr.output
          );
        return (
          expr.kind === "result" &&
          value.kind === "result" &&
          value.output === expr.output
        );
      });
      if (!symbol) throw new Error("引用不在当前作用域内");
      return symbol.type;
    }
    case "list":
      expr.items.forEach((item) => requireType(type(item), expr.item_type));
      return { type: "list", of: expr.item_type };
    case "record":
      return {
        type: "record",
        of: Object.fromEntries(
          Object.entries(expr.fields).map(([k, v]) => [k, type(v)]),
        ),
      };
    case "some":
      return { type: "optional", of: type(expr.value) };
    case "field": {
      const parent = type(expr.value);
      if (parent.type !== "record" || !parent.of[expr.field])
        throw new Error("记录没有字段：" + expr.field);
      return parent.of[expr.field];
    }
    case "index": {
      const parent = type(expr.value);
      requireType(type(expr.index), int);
      if (parent.type !== "list")
        throw new Error("只有列表可以通过整数索引访问");
      return parent.of;
    }
    case "not":
      requireType(type(expr.value), bool);
      return bool;
    case "choose": {
      requireType(type(expr.condition), bool);
      const yes = type(expr.then_value);
      requireType(type(expr.else_value), yes);
      return yes;
    }
    case "binary": {
      const left = type(expr.left),
        right = type(expr.right);
      requireType(left, right);
      if (expr.op === "and" || expr.op === "or") {
        requireType(left, bool);
        return bool;
      }
      if (expr.op === "equal" || expr.op === "not_equal") return bool;
      const comparison = [
        "less",
        "less_equal",
        "greater",
        "greater_equal",
      ].includes(expr.op);
      if (comparison && left.type === "text") return bool;
      if (left.type !== "int" && left.type !== "float")
        throw new Error("算术运算需要同类型数字；拼接文字请使用 concat");
      if (expr.op === "remainder" && left.type !== "int")
        throw new Error("余数运算只支持整数");
      return comparison ? bool : left;
    }
    case "function": {
      const args = expr.arguments.map(type),
        [a, b] = args;
      const one = args.length === 1,
        two = args.length === 2;
      switch (expr.function) {
        case "length":
          if (one && (a.type === "text" || a.type === "list")) return int;
          break;
        case "concat":
          if (two && sameType(a, b) && (a.type === "text" || a.type === "list"))
            return a;
          break;
        case "contains":
          if (
            two &&
            ((a.type === "text" && b.type === "text") ||
              (a.type === "list" && sameType(a.of, b)))
          )
            return bool;
          break;
        case "trim":
        case "lowercase":
        case "uppercase":
          if (one && a.type === "text") return text;
          break;
        case "split":
          if (two && a.type === "text" && b.type === "text")
            return { type: "list", of: text };
          break;
        case "join":
          if (
            two &&
            a.type === "list" &&
            a.of.type === "text" &&
            b.type === "text"
          )
            return text;
          break;
        case "append":
          if (two && a.type === "list" && sameType(a.of, b)) return a;
          break;
        case "is_some":
          if (one && a.type === "optional") return bool;
          break;
        case "or_else":
          if (two && a.type === "optional" && sameType(a.of, b)) return b;
          break;
        case "to_float":
          if (one && a.type === "int") return { type: "float" };
          break;
        case "to_text":
          if (one && ["int", "float", "bool", "text"].includes(a.type))
            return text;
          break;
      }
      throw new Error(expr.function + " 的参数数量或类型不匹配");
    }
  }
}
