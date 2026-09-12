import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  createWorkflow,
  listedDocuments,
  WorkflowStudio,
} from "../../../src/features/workflow";
import { testDesktopApi } from "../support/desktopApi";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it("新建立即出现在列表，非活动文档重命名保存并保留节点历史与活动标签", async () => {
  const api = testDesktopApi();
  const studio = new WorkflowStudio(api);
  await studio.initializeWorkspace();
  studio.create(createWorkflow("原名称"));
  studio.add("wait", { x: 100, y: 100 });
  const original = studio.active!;
  expect(listedDocuments(studio.store.getState())[0].name).toBe("原名称");
  studio.create(createWorkflow("当前标签"));
  const active = studio.active!.file.id;
  await studio.renameDocument(original.file.id, "  新名称  ");
  const renamed = studio.store.getState().tabs[original.file.id];
  expect(studio.active!.file.id).toBe(active);
  expect(renamed.file.definition.name).toBe("新名称");
  expect(renamed.file.definition.scopes).toBe(original.file.definition.scopes);
  expect(renamed.past.at(-1)).toBe(original.file);
  expect(renamed.status).toBe("saved");
  expect(
    studio.store
      .getState()
      .documents.find((item) => item.id === original.file.id)?.name,
  ).toBe("新名称");
  await studio.flushAll();
});

it("未打开流程使用磁盘最新版本重命名，不打开标签；空名称不写盘", async () => {
  const file = createWorkflow("关闭的流程");
  const api = testDesktopApi({
    load: vi.fn(async () => ({ file, revision: "disk" })),
  });
  const studio = new WorkflowStudio(api);
  await studio.initializeWorkspace();
  await expect(studio.renameDocument(file.id, "  ")).rejects.toThrow("请输入");
  expect(api.load).not.toHaveBeenCalled();
  await studio.renameDocument(file.id, "更名");
  expect(api.save).toHaveBeenCalledWith(
    { ...file, definition: { ...file.definition, name: "更名" } },
    "disk",
  );
  expect(studio.active).toBeUndefined();
  expect(studio.store.getState().references[file.id].definition.name).toBe(
    "更名",
  );
});

it("删除等待在途保存，清除标签与引用，后续定时器不会重新生成文档", async () => {
  let release!: () => void;
  const api = testDesktopApi({
    save: vi.fn(async (file) => {
      await new Promise<void>((resolve) => {
        release = resolve;
      });
      return { file, revision: "latest" };
    }),
  });
  const studio = new WorkflowStudio(api);
  await studio.initializeWorkspace();
  studio.create();
  const file = studio.active!.file;
  studio.store.setState({ references: { [file.id]: file } });
  const saving = studio.flushAll();
  const deleting = studio.deleteDocument(file.id);
  expect(api.deleteDocument).not.toHaveBeenCalled();
  expect(studio.readonly).toBe(true);
  release();
  await Promise.all([saving, deleting]);
  expect(api.deleteDocument).toHaveBeenCalledExactlyOnceWith(file.id, "latest");
  expect(studio.store.getState().tabs[file.id]).toBeUndefined();
  expect(studio.store.getState().references[file.id]).toBeUndefined();
  expect(studio.store.getState().documents).toEqual([]);
  expect(studio.active).toBeUndefined();
  await vi.advanceTimersByTimeAsync(1000);
  expect(api.save).toHaveBeenCalledTimes(1);
});

it("删除失败保留文档，运行涉及的非活动流程不能重命名或删除", async () => {
  const api = testDesktopApi({
    deleteDocument: vi.fn(async () => {
      throw new Error("conflict:外部修改");
    }),
  });
  const studio = new WorkflowStudio(api);
  await studio.initializeWorkspace();
  studio.create();
  const id = studio.active!.file.id;
  await expect(studio.deleteDocument(id)).rejects.toThrow("外部修改");
  expect(studio.store.getState().tabs[id]).toBeDefined();
  expect(studio.store.getState().busy).toBe(false);
  studio.create();
  studio.store.setState({
    run: {
      id: "run",
      workflow: id,
      documents: [id],
      status: "running",
      logs: [],
      omitted: 0,
      outputs: {},
      errors: [],
    },
  });
  await expect(studio.renameDocument(id, "改名")).rejects.toThrow("正在使用");
  await expect(studio.deleteDocument(id)).rejects.toThrow("正在使用");
  expect(api.deleteDocument).toHaveBeenCalledTimes(1);
  await studio.flushAll();
});
