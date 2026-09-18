import type jsep from "jsep";
import type { ObjectExpression } from "@jsep-plugin/object";
import { FUNCTIONS, type Expr, type ValueType } from "../../model/contracts";
import type { SymbolValue } from "../symbols";
import { binaryOperator } from "./operators";
import { expressionType } from "./types";

/** JSEP 的已解析 AST 只在此处转为项目契约；未列出的语法明确拒绝。 */
export function convertExpression(
  parsed: jsep.Expression,
  symbols: readonly SymbolValue[],
  expected?: ValueType,
  depth = 0,
): Expr {
  if (depth > 48) throw new Error("表达式嵌套不能超过 48 层");
  // AST 来自已注册语法的 JSEP；此断言仅补全库未判别的根类型，默认分支拒绝扩展节点。
  const node = parsed as jsep.CoreExpression | ObjectExpression;
  const convert = (value: jsep.Expression, type?: ValueType) =>
    convertExpression(value, symbols, type, depth + 1);
  const reference = path(parsed);
  if (reference) {
    const [root, first, second] = reference;
    if (root === "vars" && reference.length === 2)
      return { kind: "variable", name: first };
    if (root === "inputs" && reference.length === 2)
      return { kind: "input", name: first };
    if (root === "result" && reference.length === 2)
      return { kind: "result", output: first };
    if (root === "steps" && reference.length === 3)
      return { kind: "node_output", node: first, output: second };
  }
  switch (node.type) {
    case "Literal": {
      if (typeof node.value === "number") return numeric(node.raw, expected);
      if (typeof node.value === "string")
        return {
          kind: "literal",
          value_type: { type: "text" },
          value: {
            type: "text",
            value: node.raw.startsWith('"')
              ? (JSON.parse(node.raw) as string)
              : node.value,
          },
        };
      if (typeof node.value === "boolean")
        return {
          kind: "literal",
          value_type: { type: "bool" },
          value: { type: "bool", value: node.value },
        };
      if (node.value === null && expected?.type === "optional")
        return {
          kind: "literal",
          value_type: expected,
          value: { type: "optional", value: null },
        };
      throw new Error("null 需要明确的可选值类型");
    }
    case "Identifier":
      return { kind: "variable", name: node.name };
    case "ArrayExpression": {
      const contextual = expected?.type === "list" ? expected.of : undefined;
      const items = node.elements.map((item) => {
        if (!item) throw new Error("列表不能有空缺项");
        return convert(item, contextual);
      });
      const item_type =
        contextual ?? (items[0] && expressionType(items[0], symbols));
      if (!item_type) throw new Error("空列表需要明确的列表类型");
      return { kind: "list", item_type, items };
    }
    case "ObjectExpression": {
      const fields: Record<string, Expr> = Object.create(null);
      for (const item of node.properties) {
        if (item.computed) throw new Error("记录字段名必须固定");
        const key = propertyName(item.key);
        if (key === null) throw new Error("记录字段名必须是名称或文字");
        if (Object.hasOwn(fields, key)) throw new Error("记录字段重复：" + key);
        const value = item.shorthand ? item.key : item.value;
        if (!value) throw new Error("请填写记录字段值");
        fields[key] = convert(
          value,
          expected?.type === "record" ? expected.of[key] : undefined,
        );
      }
      return { kind: "record", fields };
    }
    case "BinaryExpression": {
      const op = binaryOperator(node.operator);
      const arithmetic = [
        "add",
        "subtract",
        "multiply",
        "divide",
        "remainder",
      ].includes(op);
      const left = convert(node.left, arithmetic ? expected : undefined);
      const right = convert(node.right, expressionType(left, symbols));
      return { kind: "binary", op, left, right };
    }
    case "UnaryExpression": {
      if (node.operator === "!")
        return { kind: "not", value: convert(node.argument, { type: "bool" }) };
      if (node.operator !== "-" && node.operator !== "+")
        throw new Error("不支持一元运算：" + node.operator);
      if (
        node.argument.type === "Literal" &&
        typeof node.argument.raw === "string" &&
        typeof node.argument.value === "number"
      )
        return numeric(
          (node.operator === "-" ? "-" : "") + node.argument.raw,
          expected,
        );
      const right = convert(node.argument, expected);
      const type = expressionType(right, symbols);
      if (type.type !== "int" && type.type !== "float")
        throw new Error("正负号只能用于数字");
      return node.operator === "+"
        ? right
        : { kind: "binary", op: "subtract", left: numeric("0", type), right };
    }
    case "ConditionalExpression": {
      const then_value = convert(node.consequent, expected);
      return {
        kind: "choose",
        condition: convert(node.test, { type: "bool" }),
        then_value,
        else_value: convert(
          node.alternate,
          expected ?? expressionType(then_value, symbols),
        ),
      };
    }
    case "MemberExpression": {
      if (node.optional) throw new Error("可选值请使用 or_else 或 is_some");
      const value = convert(node.object),
        key = propertyName(node.property);
      const parent = expressionType(value, symbols);
      if (
        parent.type === "record" &&
        node.computed &&
        node.property.type !== "Literal"
      )
        throw new Error(
          '记录字段必须使用固定名称，例如 item.name 或 item["name"]',
        );
      if (!node.computed || (parent.type === "record" && key !== null)) {
        if (key === null) throw new Error("请指定固定字段名");
        return { kind: "field", value, field: key };
      }
      return {
        kind: "index",
        value,
        index: convert(node.property, { type: "int" }),
      };
    }
    case "CallExpression": {
      if (
        node.callee.type !== "Identifier" ||
        typeof node.callee.name !== "string"
      )
        throw new Error("只支持已提供的纯函数");
      const name = node.callee.name;
      if (name === "some" && node.arguments.length === 1)
        return {
          kind: "some",
          value: convert(
            node.arguments[0],
            expected?.type === "optional" ? expected.of : undefined,
          ),
        };
      const fn = FUNCTIONS.find((item) => item === name);
      if (!fn) throw new Error("未知函数：" + name);
      return {
        kind: "function",
        function: fn,
        arguments: node.arguments.map((arg) => convert(arg)),
      };
    }
    default:
      throw new Error("这里只能填写一个值表达式，不支持语句或脚本");
  }
}
function numeric(raw: string, expected?: ValueType): Expr {
  if (/^-?\d+$/.test(raw) && expected?.type !== "float") {
    const value = BigInt(raw);
    if (value < -(2n ** 63n) || value >= 2n ** 63n)
      throw new Error("整数超出 64 位范围");
    return {
      kind: "literal",
      value_type: { type: "int" },
      value: { type: "int", value: value.toString() },
    };
  }
  const value = Number(raw);
  if (!Number.isFinite(value)) throw new Error("数字必须是有限值");
  return {
    kind: "literal",
    value_type: { type: "float" },
    value: { type: "float", value },
  };
}
function propertyName(node: jsep.Expression): string | null {
  if (node.type === "Identifier" && typeof node.name === "string")
    return node.name;
  if (node.type === "Literal" && typeof node.value === "string")
    return node.value;
  return null;
}
function path(parsed: jsep.Expression, depth = 0): readonly string[] | null {
  if (depth > 48) throw new Error("引用路径过长");
  if (parsed.type === "Identifier" && typeof parsed.name === "string")
    return [parsed.name];
  if (parsed.type !== "MemberExpression") return null;
  const node = parsed as jsep.MemberExpression;
  if (node.optional || (node.computed && node.property.type !== "Literal"))
    return null;
  const parent = path(node.object, depth + 1),
    key = propertyName(node.property);
  return parent && key !== null ? [...parent, key] : null;
}
