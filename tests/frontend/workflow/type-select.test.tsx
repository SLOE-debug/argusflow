import { useState } from "react";
import { beforeEach, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TypeSelect } from "../../../src/components/workflow/value-editor/TypeSelect";
import type { ValueType } from "../../../src/features/workflow";
import { mockUiEnvironment } from "../support/ui";

beforeEach(mockUiEnvironment);

/** 通过真实受控更新验证字段编辑和收起后的结构保留。 */
function Editor() {
  const [value, setValue] = useState<ValueType>({
    type: "list",
    of: { type: "record", of: { name: { type: "text" } } },
  });
  return <TypeSelect value={value} onChange={setValue} />;
}

it("记录列表原地展开字段，编辑后收起再打开仍保留字段", () => {
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "字段设置" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByText("每项的数据类型")).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "字段名称" })).toHaveValue("name");
  fireEvent.click(screen.getByRole("button", { name: "添加字段" }));
  expect(screen.getAllByRole("textbox", { name: "字段名称" })).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "删除字段 name" }));
  expect(screen.getByRole("textbox", { name: "字段名称" })).toHaveValue(
    "field1",
  );
  fireEvent.click(screen.getByRole("button", { name: "字段设置" }));
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "字段设置" }));
  expect(screen.getByRole("textbox", { name: "字段名称" })).toHaveValue(
    "field1",
  );
});

it("数据类型菜单可以直接选择记录列表", () => {
  render(<Editor />);
  fireEvent.click(screen.getByRole("combobox", { name: "数据类型" }));
  // 自定义结构和空结构使用同一类型名称，最后一项是可直接创建的记录列表。
  const options = screen.getAllByRole("option", { name: "记录列表" });
  fireEvent.click(options[options.length - 1]);
  fireEvent.click(screen.getByRole("button", { name: "字段设置" }));
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  expect(
    screen.getByText("还没有字段，添加需要保存的数据，例如名称、位置。"),
  ).toBeInTheDocument();
});
