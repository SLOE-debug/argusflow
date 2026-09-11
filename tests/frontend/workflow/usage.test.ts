import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  commonNodes,
  NodeUsage,
  nodeUsage,
  COMMON_NODE_DEFAULTS,
} from "../../../src/features/workflow/nodes/usage";
import { WorkflowStudio } from "../../../src/features/workflow/studio/controller";
import { testDesktopApi } from "../support/desktopApi";
import { copyNodes } from "../../../src/features/workflow/model/clipboard";

beforeEach(() => {
  localStorage.clear();
  nodeUsage.store.setState({ entries: {}, error: null });
});
afterEach(() => vi.useRealTimers());

it("常用以频次和最近顺序排序，推荐补足八项，重建服务后读回", () => {
  const usage = new NodeUsage(localStorage);
  expect(
    commonNodes(usage.store.getState().entries).map((item) => item.id),
  ).toEqual(COMMON_NODE_DEFAULTS);
  usage.record("while");
  usage.record("let");
  usage.record("while");
  expect(
    commonNodes(usage.store.getState().entries)
      .slice(0, 2)
      .map((item) => item.id),
  ).toEqual(["while", "let"]);
  usage.record("let");
  const restored = new NodeUsage(localStorage);
  expect(
    commonNodes(restored.store.getState().entries)
      .slice(0, 2)
      .map((item) => item.id),
  ).toEqual(["let", "while"]);
  expect(commonNodes(restored.store.getState().entries)).toHaveLength(8);
});

it("损坏和不可写偏好不会阻止节点使用，旧收藏不参与统计", () => {
  localStorage.setItem("argusflow.favoriteNodes", '["fail"]');
  localStorage.setItem("argusflow.nodeUsage", "{broken");
  const usage = new NodeUsage(localStorage);
  expect(usage.store.getState().error).toContain("无法读取");
  expect(
    commonNodes(usage.store.getState().entries).map((item) => item.id),
  ).not.toContain("fail");
  const blocked = new NodeUsage({
    getItem: () => null,
    setItem: () => {
      throw new Error("quota");
    },
  });
  blocked.record("while");
  expect(blocked.store.getState().entries.while.count).toBe(1);
  expect(blocked.store.getState().error).toContain("未保存");
});

it("只在添加成功时计数，失败、只读、撤销重做和副本均不增加", async () => {
  vi.useFakeTimers();
  const studio = new WorkflowStudio(testDesktopApi());
  studio.add("wait", { x: 0, y: 0 });
  expect(nodeUsage.store.getState().entries).toEqual({});
  await studio.initializeWorkspace();
  studio.create();
  expect(() => studio.add("missing-kind", { x: 0, y: 0 })).toThrow();
  studio.add("wait", { x: 0, y: 0 });
  studio.undo();
  studio.redo();
  studio.select([studio.active!.file.definition.scopes[0].nodes[0].id]);
  studio.duplicate();
  const tab = studio.active!;
  const clipboard = copyNodes(tab.file, tab.scope, new Set(tab.selected))!;
  vi.spyOn(studio.api, "paste").mockResolvedValue("clipboard");
  vi.spyOn(studio.api, "parseClipboard").mockResolvedValue(clipboard);
  await studio.paste({ x: 50, y: 50 });
  studio.store.setState({ busy: true });
  studio.add("wait", { x: 0, y: 0 });
  expect(nodeUsage.store.getState().entries.wait.count).toBe(1);
  studio.store.setState({ busy: false });
  await studio.flushAll();
});
