import { afterEach, expect, it, vi } from "vitest";
import { WorkflowStudio } from "../../../src/features/workflow/studio/controller";
import { createWorkflow } from "../../../src/features/workflow/model/factory";
import { testDesktopApi } from "../support/desktopApi";

afterEach(() => vi.useRealTimers());

it("初始化共享并发请求，已有文档自动打开，重复初始化保留草稿", async () => {
  vi.useFakeTimers();
  const file = createWorkflow();
  let release: (() => void) | undefined;
  const initialize = vi.fn(async () => {
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    return {
      path: "appdata/workflows",
      documents: [{ id: file.id, name: file.definition.name, revision: "one" }],
    };
  });
  const studio = new WorkflowStudio(
    testDesktopApi({
      initializeWorkspace: initialize,
      load: async () => ({ file, revision: "one" }),
    }),
  );
  const first = studio.initializeWorkspace(),
    second = studio.initializeWorkspace();
  expect(studio.store.getState().initialization.status).toBe("loading");
  expect(initialize).toHaveBeenCalledTimes(1);
  release!();
  await Promise.all([first, second]);
  expect(studio.active?.file.id).toBe(file.id);
  studio.edit((file) => ({
    ...file,
    definition: { ...file.definition, name: "未保存的编辑" },
  }));
  await studio.initializeWorkspace();
  expect(initialize).toHaveBeenCalledTimes(1);
  expect(studio.active?.file.definition.name).toBe("未保存的编辑");
  await studio.flushAll();
});

it("失败保留原因，重试后可直接新建，不读取旧目录偏好", async () => {
  vi.useFakeTimers();
  localStorage.setItem("argusflow.workspace", "old-path");
  const initialize = vi
    .fn()
    .mockRejectedValueOnce(new Error("目录不可写"))
    .mockResolvedValue({ path: "appdata/workflows", documents: [] });
  const studio = new WorkflowStudio(
    testDesktopApi({ initializeWorkspace: initialize }),
  );
  expect(() => studio.create()).toThrow("尚未就绪");
  await expect(studio.initializeWorkspace()).rejects.toThrow("目录不可写");
  expect(studio.store.getState().initialization).toEqual({
    status: "failed",
    error: "Error: 目录不可写",
  });
  await studio.initializeWorkspace();
  expect(initialize.mock.calls).toEqual([[], []]);
  studio.create();
  await studio.flushAll();
  expect(studio.active?.status).toBe("saved");
  expect(studio.store.getState().workspace).toBe("appdata/workflows");
  localStorage.removeItem("argusflow.workspace");
});

it("首份文档损坏不会伪装成空目录，修复后允许重试", async () => {
  const file = createWorkflow();
  const load = vi
    .fn()
    .mockRejectedValueOnce(new Error("文档损坏"))
    .mockResolvedValue({ file, revision: "one" });
  const studio = new WorkflowStudio(
    testDesktopApi({
      initializeWorkspace: async () => ({
        path: "appdata/workflows",
        documents: [{ id: file.id, name: "流程", revision: "one" }],
      }),
      load,
    }),
  );
  await expect(studio.initializeWorkspace()).rejects.toThrow("文档损坏");
  expect(studio.store.getState().initialization.status).toBe("failed");
  await studio.initializeWorkspace();
  expect(studio.store.getState().initialization.status).toBe("ready");
  expect(studio.active?.file.id).toBe(file.id);
});
