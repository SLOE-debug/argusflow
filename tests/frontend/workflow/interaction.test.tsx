import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { Canvas } from "../../../src/components/workflow/canvas/Canvas";
import { ValueField } from "../../../src/components/workflow/value-editor/ValueField";
import { Inspector } from "../../../src/components/workflow/inspector/Inspector";
import {
  studio,
  loopTemplate,
  buildScene,
  type EditorTab,
} from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { worldToScreen } from "../../../src/flow";
import {
  LIGHT_THEME,
  registerTheme,
} from "../../../src/features/themes/catalog";
import {
  initializeTheme,
  selectedTheme,
  setTheme,
  activeTheme,
  useTheme,
} from "../../../src/features/themes/controller";

function install(): EditorTab {
  const file = loopTemplate();
  const tab: EditorTab = {
    file,
    version: 0,
    savedVersion: 0,
    revision: "one",
    status: "saved",
    past: [],
    future: [],
    scope: file.definition.root,
    selected: [],
    viewport: { x: 20, y: 30, zoom: 1.46 },
  };
  studio.store.setState({
    ...INITIAL_STATE,
    workspace: "test",
    tabs: { [file.id]: tab },
    active: file.id,
  });
  return tab;
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: 1000,
    height: 700,
    right: 1000,
    bottom: 700,
    toJSON: () => ({}),
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private cb: ResizeObserverCallback) {}
      observe(target: Element) {
        this.cb(
          [
            {
              contentRect: { width: 1000, height: 700 },
              target,
            } as ResizeObserverEntry,
          ],
          this as unknown as ResizeObserver,
        );
      }
      disconnect() {}
    },
  );
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: "two",
  }));
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value() {
      this.setAttribute("open", "");
    },
  });
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value() {
      this.removeAttribute("open");
    },
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});
describe("canvas and inspector interaction without browser automation", () => {
  it("平移工具拖动画布不修改节点，选择工具恢复编辑，居中保留缩放", () => {
    const tab = install();
    vi.stubGlobal("PointerEvent", MouseEvent);
    render(<Canvas tab={tab} />);
    const canvas = screen.getByRole("application", { name: "工作流画布" });
    Object.assign(canvas, {
      setPointerCapture: vi.fn(),
      hasPointerCapture: () => false,
    });
    expect(
      screen.queryByRole("button", { name: tab.file.definition.name }),
    ).toBeNull();
    expect(screen.queryByText("146%")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "平移" }));
    expect(screen.getByRole("button", { name: "平移" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.pointerDown(canvas, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(canvas, { clientX: 180, clientY: 160 });
    fireEvent.pointerUp(canvas, { clientX: 180, clientY: 160 });
    expect(studio.active!.viewport).toEqual({ x: 100, y: 90, zoom: 1.46 });
    expect(studio.active!.file).toBe(tab.file);
    expect(studio.active!.past).toHaveLength(0);
    fireEvent.doubleClick(canvas);
    expect(screen.queryByRole("textbox", { name: "搜索节点" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "选择" }));
    expect(screen.getByRole("button", { name: "选择" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "居中显示" }));
    expect(studio.active!.viewport.zoom).toBe(tab.viewport.zoom);
    expect(studio.active!.file).toBe(tab.file);
    fireEvent.doubleClick(canvas);
    expect(
      screen.getByRole("textbox", { name: "搜索节点" }),
    ).toBeInTheDocument();
  });
  it("wheel enters only the pointed loop and exits continuously with hysteresis", () => {
    const tab = install(),
      scene = buildScene(tab.file);
    const child = Object.values(scene.scopes).find((scope) => scope.parent)!;
    render(<Canvas tab={tab} />);
    const canvas = screen.getByRole("application", { name: "工作流画布" });
    const point = worldToScreen(
      worldToScreen(
        { x: child.bounds.x + 100, y: child.bounds.y + 70 },
        child.transform,
      ),
      tab.viewport,
    );
    fireEvent.wheel(canvas, { deltaY: -1, clientX: point.x, clientY: point.y });
    expect(studio.active?.scope).toBe(child.id);
    const entered = studio.active!;
    const before = worldToScreen(
      { x: child.bounds.x + 100, y: child.bounds.y + 70 },
      entered.viewport,
    );
    expect(before.x).toBeCloseTo(point.x, 0);
    act(() => studio.view({ ...entered.viewport, zoom: 0.59 }));
    const beforeExit = studio.active!;
    fireEvent.wheel(canvas, { deltaY: 1, clientX: point.x, clientY: point.y });
    expect(studio.active?.scope).toBe(tab.scope);
    expect(studio.active!.viewport.zoom).toBeCloseTo(
      (beforeExit.viewport.zoom * Math.exp(-0.0015)) / 0.55,
    );
  });
  it("canvas shortcuts perform one transaction and input composition retains native editing", async () => {
    const tab = install();
    render(<Canvas tab={tab} />);
    const canvas = screen.getByRole("application");
    fireEvent.keyDown(canvas, { key: "a", ctrlKey: true });
    const selected = studio.active!.selected.length;
    expect(selected).toBeGreaterThan(0);
    fireEvent.keyDown(canvas, { key: "d", ctrlKey: true });
    expect(studio.active!.past).toHaveLength(1);
    fireEvent.keyDown(canvas, { key: "z", ctrlKey: true });
    expect(studio.active!.past).toHaveLength(0);
    const input = document.createElement("input");
    canvas.append(input);
    fireEvent.keyDown(input, { key: "Delete" });
    expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(
      selected,
    );
    fireEvent.keyDown(canvas, { key: "Delete", isComposing: true });
    expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(
      selected,
    );
    // 自绘控件的原生 keydown 在 React 委托处理之前也应受到保护。
    for (const role of ["combobox", "checkbox", "radio", "switch"]) {
      const control = document.createElement("button");
      control.setAttribute("role", role);
      canvas.append(control);
      fireEvent.keyDown(control, { key: "Delete" });
      expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(
        selected,
      );
    }
    await studio.flushAll();
  });
  it("reference selection never silently retains an old fixed value", () => {
    const change = vi.fn(),
      invalid = vi.fn();
    render(
      <ValueField
        value={{
          kind: "literal",
          value_type: { type: "int" },
          value: { type: "int", value: "7" },
        }}
        type={{ type: "int" }}
        symbols={[]}
        onChange={change}
        onInvalid={invalid}
      />,
    );
    fireEvent.click(screen.getByRole("combobox", { name: "取值方式" }));
    fireEvent.click(screen.getByRole("option", { name: "引用" }));
    expect(invalid).toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
    expect(screen.getByRole("combobox", { name: "引用值" })).toHaveTextContent(
      "没有可用引用",
    );
  });
  it("the inspector uses an editable title and keeps secondary execution fields collapsed", () => {
    const tab = install();
    const node = tab.file.definition.scopes[0].nodes[0];
    render(
      <Inspector tab={{ ...tab, selected: [node.id] }} onClose={() => {}} />,
    );
    expect(
      screen.getByRole("textbox", { name: "节点名称" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("基本属性")).not.toBeInTheDocument();
    const timeout = screen.getByLabelText("节点超时毫秒");
    expect(timeout.closest("details")).not.toHaveAttribute("open");
  });
});
it("an added theme and same-effective-color preference update work without component branches", () => {
  const changeHandlers: (() => void)[] = [];
  const media = {
    matches: false,
    addEventListener: (_: string, handler: () => void) =>
      changeHandlers.push(handler),
  };
  vi.stubGlobal("matchMedia", () => media);
  localStorage.removeItem("argusflow.theme");
  initializeTheme();
  function Label() {
    useTheme();
    return <span>{selectedTheme()}</span>;
  }
  render(<Label />);
  expect(screen.getByText("system")).toBeInTheDocument();
  act(() => setTheme("light"));
  expect(screen.getByText("light")).toBeInTheDocument();
  const id = "test-" + crypto.randomUUID();
  registerTheme({
    ...LIGHT_THEME,
    id,
    name: "测试主题",
    colors: { ...LIGHT_THEME.colors, accent: "#663399" },
  });
  act(() => setTheme(id));
  expect(document.documentElement.style.getPropertyValue("--af-accent")).toBe(
    "#663399",
  );
  expect(activeTheme().id).toBe(id);
  act(() => {
    setTheme("system");
    media.matches = true;
    changeHandlers.forEach((handler) => handler());
  });
  expect(activeTheme().scheme).toBe("dark");
});
