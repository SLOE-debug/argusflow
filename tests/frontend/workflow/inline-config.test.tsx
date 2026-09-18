import { beforeEach, expect, it, vi } from "vitest";
import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  compileAssignment,
  formatAssignment,
  createWorkflow,
  endpointId,
  integer,
  taskSpec,
  type EditorTab,
} from "../../../src/features/workflow";
import { Inspector } from "../../../src/components/workflow/inspector/Inspector";
import { KeyChordField } from "../../../src/components/workflow/inspector/KeyChordField";
import { FormulaEditor } from "../../../src/components/workflow/value-editor/FormulaEditor";
import { mockUiEnvironment } from "../support/ui";
vi.mock("../../../src/components/editor/monaco/CodeEditor", () => ({
  CodeEditor: ({
    source,
    onChange,
    label,
    diagnostics,
  }: {
    source: string;
    onChange: (source: string) => void;
    label: string;
    diagnostics: readonly { message: string }[];
  }) => (
    <textarea
      aria-label={label}
      data-diagnostics={diagnostics.length}
      value={source}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));
beforeEach(mockUiEnvironment);
const symbols = [
  {
    label: "行数",
    expression: { kind: "variable", name: "line" } as const,
    type: { type: "int" } as const,
  },
];
it("声明和赋值编译到原有契约，拒绝未知变量、错误类型和任意脚本", () => {
  expect(compileAssignment("line = 0", [], { type: "int" })).toEqual({
    name: "line",
    value: integer("0"),
  });
  expect(compileAssignment("line = line + 1;", symbols).value).toMatchObject({
    kind: "binary",
    op: "add",
  });
  expect(() => compileAssignment("missing = 1", symbols)).toThrow("变量未定义");
  expect(() => compileAssignment('line = "one"', symbols)).toThrow();
  expect(() => compileAssignment("line = 1; alert(1)", symbols)).toThrow();
});
it("开始节点不渲染属性内容", () => {
  const file = createWorkflow();
  const tab: EditorTab = {
    file,
    revision: null,
    version: 0,
    savedVersion: 0,
    status: "saved",
    past: [],
    future: [],
    scope: file.definition.scopes[0].id,
    selected: [endpointId(file.definition.scopes[0].id, "start")],
    selectedEdge: null,
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  expect(
    render(<Inspector tab={tab} onClose={() => {}} />).container,
  ).toBeEmptyDOMElement();
});
it("数字自增自减可编译、往返，拒绝非数字和声明中的更新", () => {
  for (const source of ["line++", "line--"]) {
    const result = compileAssignment(source + ";", symbols);
    expect(formatAssignment(result.name, result.value)).toBe(source);
    expect(result.value).toMatchObject({
      kind: "binary",
      op: source.endsWith("++") ? "add" : "subtract",
    });
  }
  const floating = [{ ...symbols[0], type: { type: "float" } as const }];
  const result = compileAssignment("line++", floating);
  expect(formatAssignment(result.name, result.value)).toBe("line++");
  expect(() => compileAssignment("missing++", symbols)).toThrow("变量未定义");
  expect(() =>
    compileAssignment("line++", [{ ...symbols[0], type: { type: "text" } }]),
  ).toThrow("数字");
  expect(() => compileAssignment("line++", symbols, { type: "int" })).toThrow(
    "初始化",
  );
});
it("组合键以标签选择，删除后不保留隐藏按键", () => {
  function Editor() {
    const [value, setValue] = useState("Control+S");
    return <KeyChordField value={value} onChange={setValue} />;
  }
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "移除 S" }));
  fireEvent.click(screen.getByRole("combobox", { name: "组合键" }));
  fireEvent.click(screen.getByRole("option", { name: "Enter" }));
  expect(
    screen.getByRole("button", { name: "移除 Control" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "移除 Enter" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "移除 S" }),
  ).not.toBeInTheDocument();
});
it("Monaco 收到实时编译错误，外部撤销更新源码", () => {
  const props = {
    symbols,
    compile: (source: string) => compileAssignment(source, symbols),
    onChange: vi.fn(),
    onInvalid: vi.fn(),
  };
  const view = render(<FormulaEditor {...props} source="line = 0" />);
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "line =" },
  });
  expect(screen.getByRole("textbox")).toHaveAttribute("data-diagnostics", "1");
  expect(props.onChange).not.toHaveBeenCalled();
  view.rerender(<FormulaEditor {...props} source="line = 2" />);
  expect(screen.getByRole("textbox")).toHaveValue("line = 2");
  expect(screen.getByRole("textbox")).toHaveAttribute("data-diagnostics", "0");
});
it("OCR 节点不向工作流暴露依赖目录", () => {
  expect(taskSpec("window.ocr")?.inputs).toEqual({});
});
