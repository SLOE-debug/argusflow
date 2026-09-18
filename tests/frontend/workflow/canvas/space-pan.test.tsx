import {
  cleanup,
  createEvent,
  fireEvent,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createWorkflow, studio } from "../../../../src/features/workflow";
import {
  canvasEnvironment,
  installCanvas,
  nestedFixture,
  nodePoint,
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

it("仅按住空格时屏蔽双击、拖入和快捷键添加，取消手势不会解除限制", () => {
  const { host } = installCanvas(createWorkflow());
  const before = studio.active!.file;
  const point = { clientX: 600, clientY: 400 };
  const drop = () => {
    const event = createEvent.drop(host, {
      dataTransfer: { getData: () => "wait" },
    });
    Object.defineProperties(event, {
      clientX: { value: point.clientX },
      clientY: { value: point.clientY },
    });
    fireEvent(host, event);
  };
  fireEvent.keyDown(host, { key: " ", code: "Space" });
  fireEvent.pointerCancel(host);
  fireEvent.doubleClick(host, point);
  fireEvent.keyDown(host, { key: "Tab" });
  fireEvent.keyDown(host, { key: "d", ctrlKey: true });
  drop();
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(studio.active!.file).toBe(before);
  fireEvent.keyUp(window, { code: "Space" });
  drop();
  expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(1);
  fireEvent.doubleClick(host, { clientX: 900, clientY: 600 });
  expect(screen.getByRole("textbox", { name: "搜索节点" })).toBeInTheDocument();
});

it.each([undefined, "out"] as const)(
  "拖动节点或端口 %s 中途按空格会取消编辑，松手不提交",
  (port) => {
    const fixture = nestedFixture();
    const { host } = installCanvas(fixture.file);
    const start = nodePoint(fixture.scope, fixture.first, port);
    const end = nodePoint(fixture.scope, fixture.second, "in");
    const before = studio.active!.file;
    fireEvent.pointerDown(host, { button: 0, ...start });
    fireEvent.pointerMove(host, end);
    fireEvent.keyDown(host, { key: " ", code: "Space" });
    fireEvent.pointerMove(host, end);
    fireEvent.pointerUp(host, end);
    expect(studio.active!.file).toBe(before);
    fireEvent.keyUp(window, { code: "Space" });
    fireEvent.pointerDown(host, { button: 0, ...start });
    fireEvent.pointerMove(host, end);
    fireEvent.pointerUp(host, end);
    expect(studio.active!.file).not.toBe(before);
  },
);

it.each([undefined, "out"] as const)(
  "按住空格从节点或端口 %s 拖动只平移",
  (port) => {
    const fixture = nestedFixture();
    const { host } = installCanvas(fixture.file);
    const start = nodePoint(fixture.scope, fixture.first, port);
    const before = studio.active!;
    fireEvent.keyDown(host, { key: " ", code: "Space" });
    fireEvent.pointerDown(host, { button: 0, ...start });
    fireEvent.pointerUp(host, {
      clientX: start.clientX + 100,
      clientY: start.clientY + 50,
    });
    expect(studio.active!.file).toBe(before.file);
    expect(studio.active!.past).toBe(before.past);
    expect(studio.active!.viewport).toEqual({ x: 140, y: 80, zoom: 1 });
  },
);
