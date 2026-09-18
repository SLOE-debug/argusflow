import { bench, describe } from "vitest";
import { compileExpressionSource } from "../../../src/features/workflow/values/source";
const symbols = [
  {
    label: "行数",
    expression: { kind: "variable", name: "line" } as const,
    type: { type: "int" } as const,
  },
  {
    label: "文件",
    expression: { kind: "input", name: "path" } as const,
    type: { type: "text" } as const,
  },
];
describe("编辑时解析与类型检查（运行时不重复执行）", () => {
  bench("变量计算 line + 1", () => {
    compileExpressionSource("line + 1", symbols, { type: "int" });
  });
  bench("复合条件", () => {
    compileExpressionSource(
      'line < 100 && contains(inputs.path, ".txt")',
      symbols,
      { type: "bool" },
    );
  });
  bench("混合引用的参数列表", () => {
    compileExpressionSource(
      '["-multiInst", "-nosession", inputs.path]',
      symbols,
      { type: "list", of: { type: "text" } },
    );
  });
});
