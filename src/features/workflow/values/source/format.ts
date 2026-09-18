import type { Expr, Value } from "../../model/contracts";
import { OPERATORS } from "./operators";
const name = (value: string) => /^[\p{L}_$][\p{L}\p{N}_$]*$/u.test(value);
const property = (value: string) =>
  name(value) ? "." + value : "[" + JSON.stringify(value) + "]";
function fixed(value: Value): string {
  switch (value.type) {
    case "text":
      return JSON.stringify(value.value);
    case "int":
      return value.value;
    case "float":
      return Number.isInteger(value.value)
        ? value.value + ".0"
        : String(value.value);
    case "bool":
      return String(value.value);
    case "optional":
      return value.value === null ? "null" : "some(" + fixed(value.value) + ")";
    case "list":
      return "[" + value.value.map(fixed).join(", ") + "]";
    case "record":
      return (
        "{ " +
        Object.entries(value.value)
          .map(([key, v]) => JSON.stringify(key) + ": " + fixed(v))
          .join(", ") +
        " }"
      );
  }
}
/** 将保存的表达式还原为可编辑源码；不执行用户文本。 */
export function formatExpression(expr: Expr): string {
  const format = formatExpression;
  switch (expr.kind) {
    case "literal":
      return fixed(expr.value);
    case "variable":
      return name(expr.name) &&
        ![
          "true",
          "false",
          "null",
          "inputs",
          "vars",
          "steps",
          "result",
        ].includes(expr.name)
        ? expr.name
        : "vars" + property(expr.name);
    case "input":
      return "inputs" + property(expr.name);
    case "node_output":
      return "steps" + property(expr.node) + property(expr.output);
    case "result":
      return "result" + property(expr.output);
    case "binary":
      return (
        "(" +
        format(expr.left) +
        " " +
        OPERATORS[expr.op] +
        " " +
        format(expr.right) +
        ")"
      );
    case "not":
      return "!(" + format(expr.value) + ")";
    case "some":
      return "some(" + format(expr.value) + ")";
    case "choose":
      return (
        "(" +
        format(expr.condition) +
        " ? " +
        format(expr.then_value) +
        " : " +
        format(expr.else_value) +
        ")"
      );
    case "list":
      return "[" + expr.items.map(format).join(", ") + "]";
    case "record":
      return (
        "{ " +
        Object.entries(expr.fields)
          .map(([key, v]) => JSON.stringify(key) + ": " + format(v))
          .join(", ") +
        " }"
      );
    case "field":
      return "(" + format(expr.value) + ")" + property(expr.field);
    case "index":
      return "(" + format(expr.value) + ")[" + format(expr.index) + "]";
    case "function":
      return expr.function + "(" + expr.arguments.map(format).join(", ") + ")";
  }
}
