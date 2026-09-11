import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  Checkbox,
  Dialog,
  FormField,
  Input,
  RadioGroup,
  Select,
  Switch,
  Textarea,
} from "../../../src/components/ui";
import { selectPlacement } from "../../../src/components/ui/select/model";
import { mockUiEnvironment } from "../support/ui";

beforeEach(mockUiEnvironment);
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
const OPTIONS = [
  { value: "one", label: "第一项" },
  { value: "disabled", label: "禁用项", disabled: true },
  { value: "three", label: "第三项" },
] as const;

it("下拉跳过禁用项，确认才提交，Escape 和 Tab 关闭且保留焦点语义", () => {
  const change = vi.fn();
  render(
    <Select
      aria-label="类型"
      value="one"
      options={OPTIONS}
      onValueChange={change}
    />,
  );
  const select = screen.getByRole("combobox");
  select.focus();
  fireEvent.keyDown(select, { key: "ArrowDown" });
  fireEvent.keyDown(select, { key: "ArrowDown" });
  expect(change).not.toHaveBeenCalled();
  expect(
    document.getElementById(select.getAttribute("aria-activedescendant")!),
  ).toHaveTextContent("第三项");
  fireEvent.keyDown(select, { key: "Enter" });
  expect(change).toHaveBeenCalledExactlyOnceWith("three");
  expect(select).toHaveFocus();
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  fireEvent.click(select);
  fireEvent.click(screen.getByRole("option", { name: "禁用项" }));
  expect(change).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(select, { key: "Escape" });
  expect(select).toHaveAttribute("aria-expanded", "false");
  fireEvent.click(select);
  fireEvent.keyDown(select, { key: "Tab" });
  expect(select).toHaveAttribute("aria-expanded", "false");
});

it("模态内的下拉挂载在对话框中，Escape 不关闭所属对话框", () => {
  const close = vi.fn();
  render(
    <Dialog title="配置" onClose={close}>
      <Select
        aria-label="类型"
        value="one"
        options={OPTIONS}
        onValueChange={vi.fn()}
      />
    </Dialog>,
  );
  const select = screen.getByRole("combobox");
  fireEvent.click(select);
  expect(screen.getByRole("listbox").closest("dialog")).toBe(
    screen.getByRole("dialog"),
  );
  fireEvent.keyDown(select, { key: "Escape" });
  expect(close).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog")).toHaveAttribute("open");
});

it("禁用和空选项不提交，外部点击关闭，监听在卸载后释放", () => {
  const change = vi.fn();
  const { rerender, unmount } = render(
    <Select
      aria-label="类型"
      value="one"
      options={OPTIONS}
      onValueChange={change}
      disabled
    />,
  );
  fireEvent.click(screen.getByRole("combobox"));
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  rerender(
    <Select
      aria-label="类型"
      value="one"
      options={[]}
      onValueChange={change}
    />,
  );
  fireEvent.click(screen.getByRole("combobox"));
  expect(screen.getByText("没有可选项")).toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
  expect(change).not.toHaveBeenCalled();
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  unmount();
  fireEvent.pointerDown(document.body);
});

it("下拉在底部翻转并限制到视口内", () => {
  const position = selectPlacement(
    { left: 970, top: 650, bottom: 682, width: 160 },
    { width: 1000, height: 700 },
    200,
  );
  expect(position.top).toBe(446);
  expect(position.left + position.width).toBeLessThanOrEqual(992);
  expect(position.maxHeight).toBe(200);
});

it("文字输入保留原生选区、组合事件和错误关联", () => {
  const composition = vi.fn(),
    change = vi.fn();
  render(
    <>
      <FormField label="名称" htmlFor="name" error="请填写名称">
        <Input
          id="name"
          aria-invalid
          defaultValue="示例文字"
          onCompositionEnd={composition}
          onChange={change}
        />
      </FormField>
      <Textarea aria-label="备注" />
    </>,
  );
  const input = screen.getByRole<HTMLInputElement>("textbox", { name: "名称" });
  input.focus();
  input.setSelectionRange(0, 2);
  expect(input.selectionEnd).toBe(2);
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "中文输入" } });
  fireEvent.compositionEnd(input, { data: "中文输入" });
  expect(composition).toHaveBeenCalledTimes(1);
  expect(change).toHaveBeenCalledTimes(1);
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(screen.getByRole("alert")).toHaveTextContent("请填写名称");
  expect(screen.getByRole("textbox", { name: "备注" }).tagName).toBe(
    "TEXTAREA",
  );
});

it("自绘复选、开关和单选通过值回调更新；单选方向键跳过禁用项", () => {
  function Controls() {
    const [checked, setChecked] = useState(false),
      [switched, setSwitched] = useState(false),
      [value, setValue] = useState("one");
    return (
      <>
        <Checkbox label="选择" checked={checked} onCheckedChange={setChecked} />
        <Switch label="启用" checked={switched} onCheckedChange={setSwitched} />
        <RadioGroup
          label="方式"
          value={value}
          options={OPTIONS}
          onValueChange={setValue}
        />
      </>
    );
  }
  render(<Controls />);
  fireEvent.click(screen.getByRole("checkbox"));
  fireEvent.click(screen.getByRole("switch"));
  expect(screen.getByRole("checkbox")).toHaveAttribute("aria-checked", "true");
  expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "true");
  fireEvent.keyDown(screen.getByRole("radio", { name: "第一项" }), {
    key: "ArrowRight",
  });
  expect(screen.getByRole("radio", { name: "第三项" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  expect(screen.getByRole("radio", { name: "第三项" })).toHaveFocus();
});
