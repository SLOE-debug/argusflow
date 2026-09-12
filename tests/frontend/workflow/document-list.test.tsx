import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DocumentList } from "../../../src/components/workflow/palette/DocumentList";
import { DocumentDialog } from "../../../src/components/workflow/palette/DocumentDialog";
import { createWorkflow, studio } from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { mockUiEnvironment } from "../support/ui";

beforeEach(() => {
  vi.useFakeTimers();
  mockUiEnvironment();
  studio.store.setState({
    ...INITIAL_STATE,
    initialization: { status: "ready" },
  });
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: "saved",
  }));
  vi.spyOn(studio.api, "deleteDocument").mockResolvedValue();
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("列表立即提供新建与打开，操作菜单重命名更新列表，删除确认后移除本地文档", async () => {
  render(<DocumentList query="" />);
  fireEvent.click(screen.getByRole("button", { name: "新建流程" }));
  const id = studio.active!.file.id;
  expect(
    screen.getByRole("button", { name: "未命名工作流" }),
  ).toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "工作流操作：未命名工作流" }),
  );
  fireEvent.click(screen.getByRole("menuitem", { name: /重命名/ }));
  expect(screen.getByRole("textbox", { name: "工作流名称" })).toHaveFocus();
  fireEvent.change(screen.getByRole("textbox", { name: "工作流名称" }), {
    target: { value: "每日流程" },
  });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "保存名称" })),
  );
  expect(screen.getByRole("button", { name: "每日流程" })).toBeInTheDocument();
  fireEvent.contextMenu(screen.getByRole("button", { name: "每日流程" }));
  fireEvent.click(screen.getByRole("menuitem", { name: /删除/ }));
  expect(studio.api.deleteDocument).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(studio.api.deleteDocument).not.toHaveBeenCalled();
  fireEvent.keyDown(screen.getByRole("button", { name: "每日流程" }), {
    key: "Delete",
  });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "删除" })),
  );
  expect(studio.api.deleteDocument).toHaveBeenCalledWith(id, "saved");
  expect(screen.queryByRole("button", { name: "每日流程" })).toBeNull();
  expect(studio.active).toBeUndefined();
});

it("删除弹窗聚焦取消按钮，N 取消、Y 确认，忽略组合键和输入法事件", async () => {
  const onClose = vi.fn();
  const remove = vi.spyOn(studio, "deleteDocument").mockResolvedValue();
  render(
    <DocumentDialog
      action={{ kind: "delete", id: "target", name: "示例" }}
      onClose={onClose}
    />,
  );
  const cancel = screen.getByRole("button", { name: "取消" });
  const confirm = screen.getByRole("button", { name: "删除" });
  expect(cancel).toHaveFocus();
  expect(cancel).toHaveAttribute("aria-keyshortcuts", "N");
  expect(confirm).toHaveAttribute("aria-keyshortcuts", "Y");
  for (const event of [
    { key: "y", ctrlKey: true },
    { key: "y", metaKey: true },
    { key: "y", altKey: true },
    { key: "y", repeat: true },
    { key: "y", isComposing: true },
  ])
    fireEvent.keyDown(cancel, event);
  expect(remove).not.toHaveBeenCalled();
  fireEvent.keyDown(cancel, { key: "n" });
  expect(onClose).toHaveBeenCalledTimes(1);
  onClose.mockClear();
  // 关闭按钮拥有焦点时，快捷键仍由整个对话框处理。
  await act(async () =>
    fireEvent.keyDown(screen.getByRole("button", { name: "关闭" }), {
      key: "Y",
    }),
  );
  expect(remove).toHaveBeenCalledExactlyOnceWith("target");
  expect(onClose).toHaveBeenCalledTimes(1);
});

it("删除等待期间 Y 不重复提交、N 不关闭，失败后允许重试", async () => {
  const onClose = vi.fn();
  let reject!: (error: Error) => void;
  const remove = vi.spyOn(studio, "deleteDocument").mockImplementation(
    () =>
      new Promise<void>((_, fail) => {
        reject = fail;
      }),
  );
  render(
    <DocumentDialog
      action={{ kind: "delete", id: "target", name: "示例" }}
      onClose={onClose}
    />,
  );
  const close = screen.getByRole("button", { name: "关闭" });
  fireEvent.keyDown(close, { key: "y" });
  fireEvent.keyDown(close, { key: "y" });
  fireEvent.keyDown(close, { key: "N" });
  expect(remove).toHaveBeenCalledTimes(1);
  expect(onClose).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "删除中…" })).toBeDisabled();
  await act(async () => reject(new Error("文件不可写")));
  expect(screen.getByRole("alert")).toHaveTextContent("文件不可写");
  remove.mockResolvedValue();
  await act(async () => fireEvent.keyDown(close, { key: "y" }));
  expect(remove).toHaveBeenCalledTimes(2);
  expect(onClose).toHaveBeenCalledTimes(1);
});

it("重命名输入的 y/n 不触发删除或关闭", () => {
  const onClose = vi.fn();
  const remove = vi.spyOn(studio, "deleteDocument");
  render(
    <DocumentDialog
      action={{ kind: "rename", id: "target", name: "示例" }}
      onClose={onClose}
    />,
  );
  const input = screen.getByRole("textbox", { name: "工作流名称" });
  expect(input).toHaveFocus();
  fireEvent.keyDown(input, { key: "y" });
  fireEvent.keyDown(input, { key: "n" });
  expect(onClose).not.toHaveBeenCalled();
  expect(remove).not.toHaveBeenCalled();
});

it("F2 表单保留失败输入，运行文档的菜单禁用修改", async () => {
  studio.create(createWorkflow("示例"));
  const file = studio.active!.file;
  render(<DocumentList query="" />);
  const rename = vi
    .spyOn(studio, "renameDocument")
    .mockRejectedValue(new Error("磁盘不可写"));
  fireEvent.keyDown(screen.getByRole("button", { name: "示例" }), {
    key: "F2",
  });
  fireEvent.change(screen.getByRole("textbox", { name: "工作流名称" }), {
    target: { value: "保留输入" },
  });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "保存名称" })),
  );
  expect(screen.getByRole("alert")).toHaveTextContent("磁盘不可写");
  expect(screen.getByRole("textbox", { name: "工作流名称" })).toHaveValue(
    "保留输入",
  );
  expect(rename).toHaveBeenCalledWith(file.id, "保留输入");
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  act(() =>
    studio.store.setState({
      run: {
        id: "run",
        workflow: file.id,
        documents: [file.id],
        status: "running",
        logs: [],
        omitted: 0,
        outputs: {},
        errors: [],
      },
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "工作流操作：示例" }));
  expect(screen.getByRole("menuitem", { name: /重命名/ })).toBeDisabled();
  expect(screen.getByRole("menuitem", { name: /删除/ })).toBeDisabled();
  expect(screen.getByRole("menuitem", { name: "打开" })).toBeEnabled();
});
