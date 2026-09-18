import { useState } from "react";
import { beforeEach, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TagSelect } from "../../../src/components/ui";
import { mockUiEnvironment } from "../support/ui";
beforeEach(mockUiEnvironment);
function Editor({ limit }: { readonly limit?: number }) {
  const [values, setValues] = useState<readonly string[]>(["Control"]);
  return (
    <TagSelect
      label="按键"
      values={values}
      onChange={setValues}
      limit={limit}
      options={["Control", "Enter", "Escape"].map((value) => ({
        value,
        label: value,
      }))}
    />
  );
}
it("同一输入框搜索并选择，保留菜单，重复选择取消标签", () => {
  render(<Editor />);
  const input = screen.getByRole("combobox", { name: "按键" });
  fireEvent.change(input, { target: { value: "esc" } });
  expect(screen.getAllByRole("option")).toHaveLength(1);
  fireEvent.click(screen.getByRole("option", { name: "Escape" }));
  expect(
    screen.getByRole("button", { name: "移除 Escape" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("listbox")).toHaveAttribute(
    "aria-multiselectable",
    "true",
  );
  expect(input).toHaveValue("");
  fireEvent.click(screen.getByRole("option", { name: "Escape" }));
  expect(
    screen.queryByRole("button", { name: "移除 Escape" }),
  ).not.toBeInTheDocument();
});
it("键盘搜索、选择、撤销，选择上限仍允许取消已选项", () => {
  render(<Editor limit={2} />);
  const input = screen.getByRole("combobox");
  fireEvent.change(input, { target: { value: "enter" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("option", { name: "Escape" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  fireEvent.click(screen.getByRole("option", { name: "Escape" }));
  expect(
    screen.queryByRole("button", { name: "移除 Escape" }),
  ).not.toBeInTheDocument();
  fireEvent.keyDown(input, { key: "Backspace" });
  expect(
    screen.queryByRole("button", { name: "移除 Enter" }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("option", { name: "Escape" })).toHaveAttribute(
    "aria-disabled",
    "false",
  );
  fireEvent.keyDown(input, { key: "Escape" });
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});
