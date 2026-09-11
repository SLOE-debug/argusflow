import {
  act,
  createEvent,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Sidebar } from "../../../src/components/workflow/palette/Sidebar";
import { Canvas } from "../../../src/components/workflow/canvas/Canvas";
import { CanvasMenu } from "../../../src/components/workflow/canvas/CanvasMenu";
import { Welcome } from "../../../src/components/workflow/workspace/Welcome";
import { StudioApp } from "../../../src/components/workflow/workspace/StudioApp";
import {
  createWorkflow,
  studio,
  nodeUsage,
} from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { mockUiEnvironment } from "../support/ui";

beforeEach(() => {
  vi.useFakeTimers();
  mockUiEnvironment();
  studio.store.setState({
    ...INITIAL_STATE,
    workspace: "appdata/workflows",
    initialization: { status: "ready" },
  });
  nodeUsage.store.setState({ entries: {}, error: null });
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: crypto.randomUUID(),
  }));
  vi.spyOn(studio.api, "listDocuments").mockResolvedValue([]);
});
afterEach(async () => {
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("侧栏默认显示节点树，搜索临时展开并恢复折叠，流程入口可打开和刷新", async () => {
  const file = createWorkflow();
  studio.store.setState({
    documents: [{ id: file.id, name: "我的流程", revision: "one" }],
  });
  const load = vi
    .spyOn(studio.api, "load")
    .mockResolvedValue({ file, revision: "one" });
  render(<Sidebar />);
  const category = screen.getByRole("treeitem", { name: "浏览器" });
  expect(category).toHaveAttribute("aria-expanded", "false");
  fireEvent.change(screen.getByRole("textbox", { name: "搜索节点库" }), {
    target: { value: "启动浏览器" },
  });
  expect(screen.getByRole("treeitem", { name: "浏览器" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  expect(
    screen.getByRole("treeitem", { name: "启动浏览器" }),
  ).toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索节点库" }), {
    target: { value: "" },
  });
  expect(screen.getByRole("treeitem", { name: "浏览器" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  fireEvent.click(screen.getByRole("tab", { name: "流程" }));
  expect(screen.queryByRole("tree")).not.toBeInTheDocument();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "我的流程" })),
  );
  expect(load).toHaveBeenCalledWith(file.id);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "刷新工作流" })),
  );
  expect(studio.api.listDocuments).toHaveBeenCalledTimes(1);
});

it("树方向键导航和 Enter 添加，拖动仅携带节点类型且不提前计数", () => {
  studio.create();
  render(<Sidebar tab={studio.active} />);
  const common = screen.getByRole("treeitem", { name: "常用" });
  common.focus();
  fireEvent.keyDown(common, { key: "ArrowRight" });
  const variable = screen.getByRole("treeitem", { name: "声明变量" });
  expect(variable).toHaveFocus();
  fireEvent.keyDown(variable, { key: "Enter" });
  expect(nodeUsage.store.getState().entries.let.count).toBe(1);
  const transfer = { setData: vi.fn(), effectAllowed: "none" };
  fireEvent.dragStart(variable, { dataTransfer: transfer });
  expect(transfer.setData).toHaveBeenCalledWith(
    "application/argusflow-node",
    "let",
  );
  expect(nodeUsage.store.getState().entries.let.count).toBe(1);
  fireEvent.keyDown(variable, { key: "ArrowLeft" });
  expect(common).toHaveFocus();
  fireEvent.keyDown(common, { key: "ArrowLeft" });
  expect(
    screen.queryByRole("treeitem", { name: "声明变量" }),
  ).not.toBeInTheDocument();
});

it("画布拖放和搜索插入各计数一次，中文输入法确认不触发添加", () => {
  studio.create();
  render(<Canvas tab={studio.active!} />);
  const canvas = screen.getByRole("application", { name: "工作流画布" });
  const drop = createEvent.drop(canvas, {
    dataTransfer: { getData: () => "wait" },
  });
  Object.defineProperties(drop, {
    clientX: { value: 100 },
    clientY: { value: 100 },
  });
  fireEvent(canvas, drop);
  expect(nodeUsage.store.getState().entries.wait.count).toBe(1);
  fireEvent.keyDown(canvas, { key: "Tab" });
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  fireEvent.change(input, { target: { value: "条件循环" } });
  fireEvent.keyDown(input, { key: "Enter", isComposing: true });
  expect(nodeUsage.store.getState().entries.while).toBeUndefined();
  fireEvent.keyDown(input, { key: "Enter" });
  expect(nodeUsage.store.getState().entries.while.count).toBe(1);
});

it("标题栏保留标签，工作流操作随编辑区显示，视图偏好集中在状态栏", async () => {
  render(<StudioApp />);
  const header = screen.getByRole("banner");
  expect(screen.queryByRole("button", { name: "运行" })).toBeNull();
  expect(screen.queryByRole("button", { name: "校验" })).toBeNull();
  expect(within(header).queryByRole("button", { name: /新建/ })).toBeNull();
  expect(screen.queryByRole("toolbar", { name: "工作流操作" })).toBeNull();
  const footer = screen.getByText("本地工作区").closest("footer")!;
  expect(
    within(footer).getByRole("combobox", { name: "主题" }),
  ).toBeInTheDocument();
  expect(
    within(footer).queryByRole("button", { name: "切换属性面板" }),
  ).toBeNull();
  const leftToggle = within(footer).getByRole("button", {
    name: "切换左侧面板",
  });
  fireEvent.click(leftToggle);
  expect(screen.queryByRole("tree")).toBeNull();
  fireEvent.click(leftToggle);
  expect(screen.getByRole("tree")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "新建工作流" }));
  const first = studio.active!.file.id;
  expect(
    within(header).getByRole("tablist", { name: "工作流标签" }),
  ).toBeInTheDocument();
  const toolbar = screen.getByRole("toolbar", { name: "工作流操作" });
  expect(toolbar.closest("header")).toBeNull();
  expect(within(toolbar).getByRole("button", { name: "运行" })).toBeEnabled();
  expect(within(toolbar).getByRole("button", { name: "校验" })).toBeEnabled();
  expect(
    within(footer).getByRole("button", { name: "切换属性面板" }),
  ).toBeInTheDocument();
  const canvas = screen.getByRole("application", { name: "工作流画布" });
  expect(within(canvas).queryByText("开始")).toBeNull();
  expect(within(canvas).queryByText("结束")).toBeNull();
  expect(
    within(canvas).getByRole("button", { name: "添加第一个节点" }),
  ).toBeInTheDocument();
  act(() => studio.add("wait", { x: 100, y: 100 }));
  expect(within(canvas).getByText("开始")).toBeInTheDocument();
  expect(within(canvas).getByText("结束")).toBeInTheDocument();
  expect(
    within(canvas).queryByRole("button", { name: "添加第一个节点" }),
  ).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "流程" }));
  fireEvent.click(screen.getByRole("button", { name: "新建流程" }));
  const second = studio.active!.file.id;
  expect(second).not.toBe(first);
  fireEvent.click(within(header).getAllByRole("tab")[0]);
  expect(studio.active!.file.id).toBe(first);
  fireEvent.keyDown(within(header).getAllByRole("tab")[0], {
    key: "ArrowRight",
  });
  expect(studio.active!.file.id).toBe(second);
  await act(async () =>
    fireEvent.click(screen.getAllByRole("button", { name: /^关闭 未命名/ })[1]),
  );
  expect(studio.store.getState().tabs[second]).toBeUndefined();
  expect(studio.active!.file.id).toBe(first);
});

it("欢迎页显示初始化错误并重试，成功后提供直接新建入口", async () => {
  studio.store.setState({
    ...INITIAL_STATE,
    initialization: { status: "failed", error: "目录不可写" },
  });
  vi.spyOn(studio.api, "initializeWorkspace").mockResolvedValue({
    path: "appdata/workflows",
    documents: [],
  });
  render(<Welcome />);
  expect(screen.getByRole("alert")).toHaveTextContent("目录不可写");
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "重试" })),
  );
  fireEvent.click(screen.getByRole("button", { name: "新建工作流" }));
  expect(studio.active).toBeDefined();
  expect(screen.queryByText("打开工作目录")).not.toBeInTheDocument();
});

it("二级节点菜单在线路落点插入并保留后继，搜索入口仍可打开完整节点库", () => {
  studio.create();
  studio.add("wait", { x: 100, y: 100 });
  studio.add("wait", { x: 500, y: 100 });
  const tab = studio.active!;
  const [first, second] = tab.file.definition.scopes[0].nodes;
  const close = vi.fn(),
    search = vi.fn();
  render(
    <CanvasMenu
      tab={tab}
      menu={{ x: 100, y: 100, point: { x: 300, y: 100 }, edge: first.id }}
      onClose={close}
      onAdd={search}
    />,
  );
  const trigger = screen.getByRole("menuitem", { name: "在线路上插入节点" });
  fireEvent.mouseEnter(trigger);
  fireEvent.click(screen.getByRole("menuitem", { name: "声明变量" }));
  const nodes = studio.active!.file.definition.scopes[0].nodes;
  const inserted = nodes.find((node) => node.action.kind === "let")!;
  expect(nodes.find((node) => node.id === first.id)!.next).toBe(inserted.id);
  expect(inserted.next).toBe(second.id);
  expect(studio.active!.file.editor.nodes[inserted.id]).toMatchObject({
    x: 300,
    y: 100,
  });
  expect(close).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("menuitem", { name: /搜索全部节点/ }));
  expect(search).toHaveBeenCalledTimes(1);
});

it("排列子菜单启用六向对齐并保持单次撤销，运行快照禁用两个编辑子菜单", () => {
  studio.create();
  studio.add("wait", { x: 100, y: 100 });
  studio.add("wait", { x: 500, y: 200 });
  studio.select(
    studio.active!.file.definition.scopes[0].nodes.map((node) => node.id),
  );
  const before = studio.active!;
  const props = {
    tab: before,
    menu: { x: 0, y: 0, point: { x: 0, y: 0 } },
    onClose: vi.fn(),
    onAdd: vi.fn(),
  };
  const { rerender } = render(<CanvasMenu {...props} />);
  fireEvent.click(screen.getByRole("menuitem", { name: "排列与对齐" }));
  expect(screen.getByRole("menuitem", { name: "水平分布" })).toBeDisabled();
  fireEvent.click(screen.getByRole("menuitem", { name: "右对齐" }));
  expect(
    Object.values(studio.active!.file.editor.nodes).map((node) => node.x),
  ).toEqual([500, 500]);
  expect(studio.active!.past.length).toBe(before.past.length + 1);
  studio.undo();
  expect(studio.active!.file).toEqual(before.file);
  vi.spyOn(studio, "readonly", "get").mockReturnValue(true);
  rerender(<CanvasMenu {...props} />);
  expect(screen.getByRole("menuitem", { name: "添加节点" })).toBeDisabled();
  expect(screen.getByRole("menuitem", { name: "排列与对齐" })).toBeDisabled();
});
