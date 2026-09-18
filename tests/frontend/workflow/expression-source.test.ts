import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  compileExpressionSource,
  formatExpression,
  sameType,
} from "../../../src/features/workflow/values/source";
import { expressionType } from "../../../src/features/workflow/values/source/types";
import { availableSymbols } from "../../../src/features/workflow/values/symbols";
import type {
  Expr,
  ValueType,
  WorkflowFile,
} from "../../../src/features/workflow/model/contracts";
import type { SymbolValue } from "../../../src/features/workflow/values/symbols";
const int: ValueType = { type: "int" },
  text: ValueType = { type: "text" },
  bool: ValueType = { type: "bool" };
const symbols: readonly SymbolValue[] = [
  { label: "行数", expression: { kind: "variable", name: "line" }, type: int },
  {
    label: "路径",
    expression: { kind: "input", name: "output_path" },
    type: text,
  },
  {
    label: "数据",
    expression: { kind: "node_output", node: "load-file", output: "rows" },
    type: { type: "list", of: { type: "record", of: { name: text } } },
  },
];
describe("源码表达式", () => {
  it("按运算优先级编译公式、条件和保留类型的引用", () => {
    const expr = compileExpressionSource("line + 2 * 3", symbols, int);
    expect(expr).toMatchObject({
      kind: "binary",
      op: "add",
      right: { kind: "binary", op: "multiply" },
    });
    expect(
      compileExpressionSource("line < 3 && line !== 2", symbols, bool),
    ).toMatchObject({ kind: "binary", op: "and" });
    expect(
      compileExpressionSource('steps["load-file"].rows[0].name', symbols, text),
    ).toMatchObject({ kind: "field", field: "name" });
  });
  it("保留 i64 边界，拒绝越界与非有限数字", () => {
    for (const source of ["9223372036854775807", "-9223372036854775808"])
      expect(compileExpressionSource(source, [], int)).toMatchObject({
        value: { type: "int", value: source },
      });
    for (const source of [
      "9223372036854775808",
      "-9223372036854775809",
      "1e999",
    ])
      expect(() => compileExpressionSource(source, [], int)).toThrow();
  });
  it("数组支持固定值与引用，空数组采用调用处类型", () => {
    const type: ValueType = { type: "list", of: text };
    expect(
      compileExpressionSource(
        '["-multiInst", inputs.output_path]',
        symbols,
        type,
      ),
    ).toMatchObject({
      kind: "list",
      items: [{ kind: "literal" }, { kind: "input", name: "output_path" }],
    });
    expect(compileExpressionSource("[]", symbols, type)).toMatchObject({
      item_type: text,
    });
    expect(() =>
      compileExpressionSource('[1, "x"]', [], { type: "list", of: int }),
    ).toThrow();
  });
  it("支持对象、纯函数和三元表达式", () => {
    const source =
      '{ name: concat("第", to_text(line)), done: line >= 3 ? true : false }';
    const expected: ValueType = {
      type: "record",
      of: { done: bool, name: text },
    };
    const expr = compileExpressionSource(source, symbols, expected);
    expect(sameType(expressionType(expr, symbols), expected)).toBe(true);
    expect(
      compileExpressionSource(formatExpression(expr), symbols, expected),
    ).toEqual(expr);
  });
  it("明确拒绝不在作用域的引用、类型错误、脚本和宿主调用", () => {
    for (const source of [
      "unknown + 1",
      'line + "1"',
      "line = 1",
      "line++;",
      "globalThis.alert(1)",
      "eval('1')",
      "line < 3; line + 1",
      "true + false",
      "1.5 % 1.0",
    ])
      expect(() => compileExpressionSource(source, symbols, int)).toThrow();
    expect(() =>
      compileExpressionSource("(".repeat(49) + "1" + ")".repeat(49), [], int),
    ).toThrow(/48/);
  });
  it("还原文字不破坏路径、引号、控制字符和 unicode 转义", () => {
    const expr: Expr = {
      kind: "literal",
      value_type: text,
      value: { type: "text", value: 'C:\\路径\\a"b\n\u0000' },
    };
    expect(compileExpressionSource(formatExpression(expr), [], text)).toEqual(
      expr,
    );
  });
  it("现有 Notepad workflow 的所有输入公式都可编辑后重新检查类型", () => {
    const file: WorkflowFile = JSON.parse(
      readFileSync(
        "tests/argusflow-workflow-automation/fixtures/notepad-ocr-while.workflow.json",
        "utf8",
      ),
    );
    let count = 0;
    for (const scope of file.definition.scopes)
      for (const node of scope.nodes) {
        const visible = availableSymbols(file, scope.id, node.id).values;
        const action = node.action;
        const entries =
          action.kind === "task"
            ? Object.values(action.task.inputs)
            : action.kind === "let"
              ? [action.value]
              : action.kind === "while"
                ? [action.condition]
                : action.kind === "assign"
                  ? action.assignments.map((a) => a.value)
                  : [];
        for (const expr of entries) {
          const type = expressionType(expr, visible);
          expect(
            sameType(
              expressionType(
                compileExpressionSource(formatExpression(expr), visible, type),
                visible,
              ),
              type,
            ),
          ).toBe(true);
          count++;
        }
      }
    expect(count).toBeGreaterThan(0);
  });
});
