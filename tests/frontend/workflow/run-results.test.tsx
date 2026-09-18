import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RunResults } from "../../../src/components/workflow/execution/results";
import { DataPanel } from "../../../src/components/workflow/data/DataPanel";
import { Dock } from "../../../src/components/workflow/workspace/Dock";
import { ResultContent } from "../../../src/components/workflow/execution/results/ResultContent";
import { resultClipboard } from "../../../src/components/workflow/execution/results/valuePresentation";
import {
  createWorkflow,
  studio,
  type EditorTab,
  type RunSnapshot,
} from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { mockUiEnvironment } from "../support/ui";

const file = createWorkflow();
const tab: EditorTab = {
  file,
  version: 0,
  savedVersion: 0,
  revision: null,
  status: "saved",
  past: [],
  future: [],
  scope: file.definition.root,
  selected: [],
  selectedEdge: null,
  viewport: { x: 0, y: 0, zoom: 1 },
};
const run: RunSnapshot = {
  id: "run-1",
  workflow: file.id,
  documents: [file.id],
  status: "completed",
  logs: [],
  omitted: 0,
  errors: [],
  outputs: {
    保存位置: { type: "text", value: "D:/导出/报告.txt" },
    已保存: { type: "bool", value: true },
  },
};
beforeEach(() => {
  mockUiEnvironment();
  studio.store.setState({
    ...INITIAL_STATE,
    run,
    dock: "results",
    dockOpen: true,
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("流程设置不包含实际运行结果，运行结果有独立入口", () => {
  const settings = render(<DataPanel tab={tab} />);
  expect(screen.getByRole("tab", { name: "开始前填写" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("tab", { name: "结果设置" }));
  expect(screen.queryByText("D:/导出/报告.txt")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("region", { name: "运行结果" }),
  ).not.toBeInTheDocument();
  settings.unmount();
  render(<Dock tab={tab} height={400} onHeight={() => {}} />);
  expect(screen.getByRole("tab", { name: "运行结果" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(screen.getByRole("tab", { name: "流程设置" })).toBeInTheDocument();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

it("结果列表切换独立详情，复制正文不携带类型外壳", () => {
  const copy = vi.spyOn(studio.api, "copy").mockResolvedValue();
  render(<RunResults workflow={file.id} />);
  fireEvent.click(screen.getByRole("button", { name: "复制内容" }));
  expect(copy).toHaveBeenCalledWith("D:/导出/报告.txt");
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "结果列表" })).getByRole(
      "button",
      { name: /已保存/ },
    ),
  );
  expect(screen.getByRole("heading", { name: "已保存" })).toBeInTheDocument();
  expect(document.querySelector("pre")).toBeNull();
});

it("不同流程不显示上一次其他流程的结果，运行中不伪装为完成", () => {
  const view = render(<RunResults workflow="another" />);
  expect(screen.getByText("还没有运行结果")).toBeInTheDocument();
  studio.store.setState({ run: { ...run, status: "running", outputs: {} } });
  view.rerender(<RunResults workflow={file.id} />);
  expect(screen.getByText("正在运行")).toBeInTheDocument();
  expect(screen.getByText("结果将在运行结束后显示")).toBeInTheDocument();
});

it("多项记录展示为表格，大整数复制不丢失精度", () => {
  render(
    <ResultContent
      value={{
        type: "list",
        value: [
          {
            type: "record",
            value: {
              名称: { type: "text", value: "第一项" },
              数量: { type: "int", value: "9007199254740993" },
            },
          },
          {
            type: "record",
            value: { 名称: { type: "text", value: "第二项" } },
          },
        ],
      }}
    />,
  );
  expect(screen.getByRole("table")).toBeInTheDocument();
  expect(
    screen.getByRole("columnheader", { name: "名称" }),
  ).toBeInTheDocument();
  expect(screen.getByText("9007199254740993")).toBeInTheDocument();
  expect(resultClipboard({ type: "int", value: "9007199254740993" })).toBe(
    "9007199254740993",
  );
});
