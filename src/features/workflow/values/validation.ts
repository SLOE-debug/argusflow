import {
  BINARY_OPS,
  FUNCTIONS,
  type Expr,
  type Value,
  type ValueType,
} from "../model/contracts";
function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
/** 所有高级编辑器必须先验证未知 JSON，再写入强类型文档。 */
export function isValueType(value: unknown, depth = 0): value is ValueType {
  if (depth > 48 || !record(value)) return false;
  if (["text", "int", "float", "bool"].includes(String(value.type)))
    return true;
  if (value.type === "list" || value.type === "optional")
    return isValueType(value.of, depth + 1);
  return (
    value.type === "record" &&
    record(value.of) &&
    Object.values(value.of).every((item) => isValueType(item, depth + 1))
  );
}
export function isValue(value: unknown, depth = 0): value is Value {
  if (depth > 64 || !record(value)) return false;
  switch (value.type) {
    case "text":
      return typeof value.value === "string";
    case "bool":
      return typeof value.value === "boolean";
    case "int":
      return (
        typeof value.value === "string" &&
        /^-?\d+$/.test(value.value) &&
        BigInt(value.value) >= -(2n ** 63n) &&
        BigInt(value.value) < 2n ** 63n
      );
    case "float":
      return typeof value.value === "number" && Number.isFinite(value.value);
    case "list":
      return (
        Array.isArray(value.value) &&
        value.value.every((item) => isValue(item, depth + 1))
      );
    case "record":
      return (
        record(value.value) &&
        Object.values(value.value).every((item) => isValue(item, depth + 1))
      );
    case "optional":
      return value.value === null || isValue(value.value, depth + 1);
    default:
      return false;
  }
}
export function isExpr(value: unknown, depth = 0): value is Expr {
  if (depth > 48 || !record(value)) return false;
  const expr = (item: unknown) => isExpr(item, depth + 1);
  switch (value.kind) {
    case "literal":
      return isValueType(value.value_type) && isValue(value.value);
    case "variable":
    case "input":
      return typeof value.name === "string";
    case "node_output":
      return typeof value.node === "string" && typeof value.output === "string";
    case "result":
      return typeof value.output === "string";
    case "field":
      return expr(value.value) && typeof value.field === "string";
    case "index":
      return expr(value.value) && expr(value.index);
    case "binary":
      return (
        BINARY_OPS.some((op) => op === value.op) &&
        expr(value.left) &&
        expr(value.right)
      );
    case "not":
    case "some":
      return expr(value.value);
    case "choose":
      return (
        expr(value.condition) &&
        expr(value.then_value) &&
        expr(value.else_value)
      );
    case "list":
      return (
        isValueType(value.item_type) &&
        Array.isArray(value.items) &&
        value.items.every(expr)
      );
    case "record":
      return record(value.fields) && Object.values(value.fields).every(expr);
    case "function":
      return (
        FUNCTIONS.some((fn) => fn === value.function) &&
        Array.isArray(value.arguments) &&
        value.arguments.every(expr)
      );
    default:
      return false;
  }
}
export function parseExpression(source: string): Expr {
  const value: unknown = JSON.parse(source);
  if (!isExpr(value))
    throw new Error("表达式结构无效，请检查类型、运算符和字段");
  return value;
}
