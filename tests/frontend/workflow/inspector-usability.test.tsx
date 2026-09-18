vi.mock("../../../src/components/editor/monaco/CodeEditor", () => ({
  CodeEditor: ({
    source,
    onChange,
    label,
  }: {
    source: string;
    onChange: (source: string) => void;
    label: string;
  }) => (
    <textarea
      aria-label={label}
      value={source}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { ListExpressionEditor } from "../../../src/components/workflow/value-editor/ListExpressionEditor";
import { mockUiEnvironment } from "../support/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { ExpressionDialog } from "../../../src/components/workflow/value-editor/ExpressionDialog";
import { ExpressionInput } from "../../../src/components/workflow/value-editor/ExpressionInput";
import { LaunchArguments } from "../../../src/components/workflow/inspector/LaunchArguments";
import { availableSymbols } from "../../../src/features/workflow/values/symbols";
import { presentNodes } from "../../../src/components/workflow/canvas/scene";
import {
  integer,
  text,
} from "../../../src/features/workflow/model/expressions";
import type {
  WorkflowFile,
  Expr,
} from "../../../src/features/workflow/model/contracts";
const file: WorkflowFile = JSON.parse(
  readFileSync(
    "tests/argusflow-workflow-automation/fixtures/notepad-ocr-while.workflow.json",
    "utf8",
  ),
);
const symbols = [
  {
    label: "变量 · line",
    expression: { kind: "variable", name: "line" } as const,
    type: { type: "int" } as const,
  },
];
describe("节点配置可读性", () => {
  beforeEach(mockUiEnvironment);
  it("自定义名称不遮住画布节点类型", () => {
    expect(presentNodes(file).get("reserve")?.summary).toContain("新建空文件");
  });
  it("资源显示来源节点，保留已有绑定身份", () => {
    const resources = availableSymbols(file, "root", "activate").resources;
    expect(resources.find((r) => r.name === "npp")?.label).toBe(
      "打开 Notepad++ · 应用",
    );
    expect(resources.find((r) => r.name === "editor")?.label).toBe(
      "等待编辑器窗口 · 窗口",
    );
  });
  it("表达式弹窗显示源码，错误输入不提交", () => {
    const apply = vi.fn();
    render(
      <ExpressionDialog
        value={integer("0")}
        type={{ type: "int" }}
        symbols={symbols}
        onApply={apply}
        onClose={() => {}}
      />,
    );
    const input = screen.getByRole("textbox", { name: "表达式" });
    expect(input).toHaveValue("0");
    expect(screen.queryByLabelText("表达式 JSON")).not.toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'line + "1"' } });
    fireEvent.click(screen.getByRole("button", { name: "应用表达式" }));
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(apply).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "line + 1" } });
    fireEvent.click(screen.getByRole("button", { name: "应用表达式" }));
    expect(apply).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "binary", op: "add" }),
    );
  });
  it("无效公式保留草稿，修复后提交类型化表达式", () => {
    const change = vi.fn(),
      invalid = vi.fn();
    render(
      <ExpressionInput
        value={integer("0")}
        pending="line +"
        type={{ type: "int" }}
        symbols={symbols}
        onChange={change}
        onInvalid={invalid}
      />,
    );
    const input = screen.getByLabelText("值表达式");
    expect(screen.getByRole("alert")).toBeInTheDocument();
    fireEvent.blur(input);
    expect(input).toHaveValue("line +");
    fireEvent.change(input, { target: { value: "line + 1" } });
    expect(change).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "binary" }),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("启动参数开放输入且保留独立文件引用", () => {
    const change = vi.fn();
    const args: Expr = {
      kind: "list",
      item_type: { type: "text" },
      items: [
        text("-multiInst"),
        text("-nosession"),
        text("-noPlugin"),
        { kind: "input", name: "output_path" },
      ],
    };
    render(
      <LaunchArguments
        value={args}
        symbols={[
          {
            label: "输入 · 保存路径",
            expression: { kind: "input", name: "output_path" },
            type: { type: "text" },
          },
        ]}
        onChange={change}
        onInvalid={() => {}}
      />,
    );
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("-multiInst")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "引用值" })).toHaveTextContent(
      "输入 · 保存路径",
    );
    fireEvent.change(screen.getByDisplayValue("-noPlugin"), {
      target: { value: "--custom-option" },
    });
    expect(change).toHaveBeenCalledWith({
      ...args,
      items: [
        text("-multiInst"),
        text("-nosession"),
        text("--custom-option"),
        { kind: "input", name: "output_path" },
      ],
    });
  });
  it("删除列表前项后保留后一项的无效草稿", () => {
    function Editor() {
      const [items, setItems] = useState<readonly Expr[]>([
        integer("1"),
        integer("2"),
      ]);
      return (
        <ListExpressionEditor
          items={items}
          itemType={{ type: "int" }}
          symbols={[]}
          depth={0}
          onChange={setItems}
          onInvalid={() => {}}
        />
      );
    }
    render(<Editor />);
    fireEvent.change(screen.getAllByLabelText("固定值")[1], {
      target: { value: "unfinished" },
    });
    fireEvent.click(screen.getByRole("button", { name: "删除第 1 项" }));
    expect(screen.getByLabelText("固定值")).toHaveValue("unfinished");
  });
});
