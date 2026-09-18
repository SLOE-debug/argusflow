import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useStore } from "zustand";
import {
  createNode,
  createWorkflow,
  replaceScope,
  scopeById,
  studio,
  taskSpec,
  type EditorTab,
} from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { DataPanel } from "../../../src/components/workflow/data/DataPanel";
import { mockUiEnvironment } from "../support/ui";

/** 使用真实编辑事务验证来源选择和撤销，不依赖浏览器布局。 */
function Panel() {
  const tab = useStore(studio.store, (state) => state.tabs[state.active!]);
  return <DataPanel tab={tab} />;
}

beforeEach(() => {
  mockUiEnvironment();
  vi.useFakeTimers();
  const file = createWorkflow();
  const query = createNode("aql.query").node;
  const check = createNode("file.wait_text").node;
  const root = scopeById(file, file.definition.root);
  const next = replaceScope(file, {
    ...root,
    nodes: [query, check],
    edges: [
      {
        id: "a",
        source: { kind: "start" },
        target: { kind: "node", node: query.id },
      },
      {
        id: "b",
        source: { kind: "node", node: query.id },
        target: { kind: "node", node: check.id },
      },
      {
        id: "c",
        source: { kind: "node", node: check.id },
        target: { kind: "end" },
      },
    ],
  });
  const tab: EditorTab = {
    file: next,
    version: 0,
    savedVersion: 0,
    revision: null,
    status: "saved",
    past: [],
    future: [],
    scope: root.id,
    selected: [],
    selectedEdge: null,
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  studio.store.setState({
    ...INITIAL_STATE,
    tabs: { [file.id]: tab },
    active: file.id,
  });
});

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function addQueryResult() {
  fireEvent.click(screen.getByRole("tab", { name: "结果设置" }));
  fireEvent.click(screen.getByRole("button", { name: "添加结果" }));
  fireEvent.click(screen.getByRole("combobox", { name: "选择要显示的内容" }));
  fireEvent.click(screen.getByRole("option", { name: /找到的内容/ }));
}

it("直接选择找到的内容，自动保存完整记录列表，无需配置字段", () => {
  render(<Panel />);
  addQueryResult();
  const file = studio.active!.file;
  expect(file.definition.outputs["找到的内容"]).toEqual(
    taskSpec("aql.query")!.outputs.matches,
  );
  expect(
    scopeById(file, file.definition.root).outputs["找到的内容"],
  ).toMatchObject({ kind: "node_output", output: "matches" });
  expect(screen.getByRole("textbox", { name: "结果名称" })).toHaveValue(
    "找到的内容",
  );
  expect(
    screen.queryByRole("combobox", { name: "数据类型" }),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("更换结果来源自动更新类型，保留名称，支持撤销", () => {
  render(<Panel />);
  addQueryResult();
  fireEvent.click(screen.getByRole("combobox", { name: "显示什么内容" }));
  fireEvent.click(screen.getByRole("option", { name: /文件内容是否一致/ }));
  expect(studio.active!.file.definition.outputs["找到的内容"]).toEqual({
    type: "bool",
  });
  expect(
    scopeById(studio.active!.file, studio.active!.file.definition.root).outputs[
      "找到的内容"
    ],
  ).toMatchObject({ output: "equal" });
  act(() => studio.undo());
  expect(studio.active!.file.definition.outputs["找到的内容"]).toEqual(
    taskSpec("aql.query")!.outputs.matches,
  );
});

it("取消添加不会创建空结果，重复选择会自动区分名称", () => {
  render(<Panel />);
  fireEvent.click(screen.getByRole("tab", { name: "结果设置" }));
  fireEvent.click(screen.getByRole("button", { name: "添加结果" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(studio.active!.file.definition.outputs).toEqual({});
  addQueryResult();
  addQueryResult();
  expect(Object.keys(studio.active!.file.definition.outputs)).toEqual([
    "找到的内容",
    "找到的内容2",
  ]);
});

it("手动填写仍可用，不要求先选择数据类型", () => {
  render(<Panel />);
  fireEvent.click(screen.getByRole("tab", { name: "结果设置" }));
  fireEvent.click(screen.getByRole("button", { name: "添加结果" }));
  fireEvent.click(screen.getByRole("button", { name: "手动填写内容" }));
  fireEvent.change(screen.getByRole("textbox", { name: "固定值" }), {
    target: { value: "处理完成" },
  });
  expect(
    scopeById(studio.active!.file, studio.active!.file.definition.root).outputs[
      "结果"
    ],
  ).toMatchObject({
    kind: "literal",
    value: { type: "text", value: "处理完成" },
  });
});

it("已删除的来源明确提示重新选择", () => {
  render(<Panel />);
  addQueryResult();
  act(() =>
    studio.edit((file) => {
      const root = scopeById(file, file.definition.root);
      return replaceScope(file, {
        ...root,
        nodes: [],
        edges: [
          { id: "empty", source: { kind: "start" }, target: { kind: "end" } },
        ],
      });
    }),
  );
  expect(
    screen.getByText("原来选择的内容已不可用，请重新选择。"),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("combobox", { name: "显示什么内容" }),
  ).toHaveAttribute("aria-invalid", "true");
});
