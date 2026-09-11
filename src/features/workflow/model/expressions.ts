import type { Expr, Value, ValueType } from "./contracts";
export const TEXT: ValueType = { type: "text" };
export const INT: ValueType = { type: "int" };
export const BOOL: ValueType = { type: "bool" };
export const FLOAT: ValueType = { type: "float" };
/** 创建带类型的常量。 */
export function literal(value_type: ValueType, value: Value): Expr {
  return { kind: "literal", value_type, value };
}
export function text(value: string): Expr {
  return literal(TEXT, { type: "text", value });
}
export function integer(value: string): Expr {
  return literal(INT, { type: "int", value });
}
export function boolean(value: boolean): Expr {
  return literal(BOOL, { type: "bool", value });
}
export function defaultValue(type: ValueType): Value {
  switch (type.type) {
    case "text":
      return { type: "text", value: "" };
    case "int":
      return { type: "int", value: "0" };
    case "float":
      return { type: "float", value: 0 };
    case "bool":
      return { type: "bool", value: false };
    case "list":
      return { type: "list", value: [] };
    case "optional":
      return { type: "optional", value: null };
    case "record":
      return {
        type: "record",
        value: Object.fromEntries(
          Object.entries(type.of).map(([key, item]) => [
            key,
            defaultValue(item),
          ]),
        ),
      };
  }
}
export function expressionLabel(expr: Expr): string {
  switch (expr.kind) {
    case "literal":
      return expr.value.type === "text"
        ? expr.value.value || "未填写"
        : JSON.stringify(expr.value.value);
    case "variable":
      return expr.name;
    case "input":
      return "输入." + expr.name;
    case "node_output":
      return expr.node + "." + expr.output;
    case "result":
      return "结果." + expr.output;
    case "binary":
      return (
        expressionLabel(expr.left) +
        " " +
        expr.op +
        " " +
        expressionLabel(expr.right)
      );
    case "list":
      return expr.items.length + " 项";
    case "record":
      return Object.keys(expr.fields).length + " 个字段";
    case "field":
      return expressionLabel(expr.value) + "." + expr.field;
    case "index":
      return (
        expressionLabel(expr.value) + "[" + expressionLabel(expr.index) + "]"
      );
    case "not":
      return "非 " + expressionLabel(expr.value);
    case "some":
      return expressionLabel(expr.value);
    case "choose":
      return "条件取值";
    case "function":
      return expr.function + "(…)";
  }
}
export function typeLabel(type: ValueType): string {
  switch (type.type) {
    case "text":
      return "文字";
    case "int":
      return "整数";
    case "float":
      return "小数";
    case "bool":
      return "布尔";
    case "list":
      return typeLabel(type.of) + "列表";
    case "optional":
      return "可选" + typeLabel(type.of);
    case "record":
      return "记录";
  }
}
