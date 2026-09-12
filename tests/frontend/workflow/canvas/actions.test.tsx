import { createConnection } from "../../../../src/features/workflow";
import { nextNode } from "../../support/graph";
import {
  act,
  cleanup,
  createEvent,
  fireEvent,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  createWorkflow,
  nodeById,
  studio,
} from "../../../../src/features/workflow";
import { copyNodes } from "../../../../src/features/workflow/model/clipboard";
import {
  canvasEnvironment,
  installCanvas,
  localPoint,
  nestedFixture,
} from "../../support/canvasFixture";

beforeEach(() => {
  vi.useFakeTimers();
  canvasEnvironment();
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("空画布保持空白并直接接受拖入", () => {
  const { host: target } = installCanvas(createWorkflow());
  expect(screen.queryByText("从一个步骤开始")).toBeNull();
  const over = createEvent.dragOver(target, {
    dataTransfer: { types: ["application/argusflow-node"], dropEffect: "none" },
  });
  fireEvent(target, over);
  expect(over.defaultPrevented).toBe(true);
  const drop = createEvent.drop(target, {
    dataTransfer: { getData: () => "wait" },
  });
  Object.defineProperties(drop, {
    clientX: { value: 600 },
    clientY: { value: 400 },
  });
  fireEvent(target, drop);
  expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(1);
  expect(
    studio.active!.file.editor.nodes[studio.active!.selected[0]],
  ).toMatchObject({ x: 560, y: 370 });
});

it("Tab 打开搜索后聚焦输入框，弹窗默认焦点不会抢走它", () => {
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
      this.querySelector<HTMLButtonElement>("button")?.focus();
    },
  );
  const { host } = installCanvas(createWorkflow());
  host.focus();
  fireEvent.keyDown(host, { key: "Tab" });
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  expect(input).toHaveFocus();
  fireEvent.change(input, { target: { value: "等待" } });
  expect(input).toHaveFocus();
  fireEvent.keyDown(input, { key: "Enter" });
  expect(studio.active!.file.definition.scopes[0].nodes[0].action.kind).toBe(
    "wait",
  );
  expect(host).toHaveFocus();
});

it("子流程拖入节点将屏幕落点转换为所属作用域局部坐标", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  const point = localPoint(fixture.scope, { x: 350, y: 60 });
  const drop = createEvent.drop(host, {
    dataTransfer: { getData: () => "let" },
  });
  Object.defineProperties(drop, {
    clientX: { value: point.clientX },
    clientY: { value: point.clientY },
  });
  fireEvent(host, drop);
  const id = studio.active!.selected[0];
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(studio.active!.file.editor.nodes[id].x).toBeCloseTo(350);
  expect(studio.active!.file.editor.nodes[id].y).toBeCloseTo(60);
  expect(
    studio.active!.file.definition.scopes.find(
      (scope) => scope.id === fixture.scope,
    )!.nodes,
  ).toHaveLength(3);
});

it("搜索与右键菜单保留打开时的目标作用域，关闭菜单不关闭搜索", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  fireEvent.contextMenu(host, localPoint(fixture.scope, { x: 350, y: 20 }));
  fireEvent.mouseEnter(screen.getByRole("menuitem", { name: "添加节点" }));
  fireEvent.click(screen.getByRole("menuitem", { name: /搜索全部节点/ }));
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  act(() => studio.select([], fixture.root));
  fireEvent.change(input, { target: { value: "声明变量" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(studio.active!.scope).toBe(fixture.scope);
  const node = studio.active!.selected[0];
  expect(studio.active!.file.editor.nodes[node].x).toBeCloseTo(350);
  expect(nodeById(studio.active!.file, node)!.action.kind).toBe("let");
});

it("子流程连线上双击插入保留原后继", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  act(() =>
    studio.edit((file) =>
      createConnection(file, fixture.scope, fixture.first, fixture.second),
    ),
  );
  const edge = buildCanvasScene(studio.active!.file).details[
    fixture.scope
  ].edges.find((edge) => edge.source === fixture.first)!;
  const segment = edge.segments.find(
    (segment) =>
      segment.kind === "line" &&
      Math.hypot(segment.to.x - segment.from.x, segment.to.y - segment.from.y) >
        30,
  )!;
  const point = {
    x: (segment.from.x + segment.to.x) / 2,
    y: (segment.from.y + segment.to.y) / 2,
  };
  fireEvent.doubleClick(host, localPoint(fixture.scope, point));
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  fireEvent.change(input, { target: { value: "声明变量" } });
  fireEvent.keyDown(input, { key: "Enter" });
  const inserted = studio.active!.selected[0];
  expect(nextNode(studio.active!.file, fixture.first, fixture.scope)).toBe(
    inserted,
  );
  expect(nextNode(studio.active!.file, inserted, fixture.scope)).toBe(
    fixture.second,
  );
});

it("异步粘贴绑定显式目标，等待期间选择父节点不会改变归属", async () => {
  const fixture = nestedFixture();
  installCanvas(fixture.file);
  const clipboard = copyNodes(
    fixture.file,
    fixture.scope,
    new Set([fixture.first]),
  )!;
  let resolve!: (value: string) => void;
  vi.spyOn(studio.api, "paste").mockImplementation(
    () =>
      new Promise<string>((done) => {
        resolve = done;
      }),
  );
  vi.spyOn(studio.api, "parseClipboard").mockResolvedValue(clipboard);
  let pending!: Promise<void>;
  act(() => {
    pending = studio.paste({ x: -100, y: 200 }, fixture.scope);
    studio.select([fixture.outer], fixture.root);
  });
  await act(async () => {
    resolve("clipboard");
    await pending;
  });
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(
    studio.active!.file.editor.nodes[studio.active!.selected[0]],
  ).toMatchObject({ x: -100, y: 200 });
});
