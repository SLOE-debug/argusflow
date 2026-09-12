import { render, screen } from "@testing-library/react";
import { useStore } from "zustand";
import { vi } from "vitest";
import { Canvas } from "../../../src/components/workflow/canvas/Canvas";
import {
  buildCanvasScene,
  childScopes,
  createWorkflow,
  endpointId,
  setLayout,
  nodeById,
  studio,
  type WorkflowFile,
} from "../../../src/features/workflow";
import { addNode } from "../../../src/features/workflow/model/graph";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { compose, worldToScreen, type FlowPoint } from "../../../src/flow";
import { mockCanvas } from "./canvas";
import { mockUiEnvironment } from "./ui";

/** 两层流程提供同层连接、跨层拒绝和容器外拖动场景。 */
export function nestedFixture(depth = 1) {
  let file = createWorkflow("画布测试");
  const root = file.definition.root;
  const outer = addNode(file, root, "wait", { x: 80, y: 100 });
  file = outer.file;
  let scope = root;
  const containers: string[] = [];
  for (let level = 0; level < depth; level++) {
    const created = addNode(file, scope, "while", {
      x: level ? 80 : 420,
      y: 100,
    });
    file = created.file;
    containers.push(created.id);
    scope = childScopes(nodeById(file, created.id)!.action)[0].id;
  }
  const first = addNode(file, scope, "wait", { x: 80, y: 80 }, null);
  const second = addNode(first.file, scope, "wait", { x: 440, y: 80 }, null);
  return {
    file: setLayout(second.file, endpointId(scope, "end"), { x: 800, y: 104 }),
    root,
    scope,
    first: first.id,
    second: second.id,
    outer: outer.id,
    containers,
  };
}

/** jsdom 仅模拟布局与绘图上下文，不启动浏览器。 */
export function canvasEnvironment() {
  mockUiEnvironment();
  const recording = mockCanvas();
  vi.stubGlobal("PointerEvent", MouseEvent);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1200);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: 1200,
    height: 800,
    right: 1200,
    bottom: 800,
    toJSON: () => ({}),
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly callback: ResizeObserverCallback) {}
      observe(target: Element) {
        this.callback(
          [
            {
              target,
              contentRect: { width: 1200, height: 800 },
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
    revision: "saved",
  }));
  return recording;
}

export function installCanvas(file: WorkflowFile) {
  studio.store.setState({
    ...INITIAL_STATE,
    initialization: { status: "ready" },
  });
  studio.create(file);
  studio.view({ x: 40, y: 30, zoom: 1 });
  function Harness() {
    const state = useStore(studio.store);
    return <Canvas tab={state.tabs[state.active!]} />;
  }
  const rendered = render(<Harness />);
  const host = screen.getByRole("application", { name: "工作流画布" });
  Object.assign(host, {
    setPointerCapture: vi.fn(),
    hasPointerCapture: () => false,
    releasePointerCapture: vi.fn(),
  });
  return { ...rendered, host };
}

/** 将显式局部坐标投影为鼠标事件坐标。 */
export function localPoint(scope: string, point: FlowPoint) {
  const tab = studio.active!;
  const transform = buildCanvasScene(tab.file).scopes[scope].transform;
  const screen = worldToScreen(point, compose(tab.viewport, transform));
  return { clientX: screen.x, clientY: screen.y };
}

export function nodePoint(scope: string, id: string, port?: "in" | "out") {
  const geometry = buildCanvasScene(studio.active!.file).scopes[
    scope
  ].nodes.find((node) => node.id === id)!;
  return localPoint(scope, {
    x: geometry.x + (port === "in" ? 0 : port === "out" ? geometry.width : 70),
    y:
      geometry.y +
      (geometry.children.length && !port ? 20 : geometry.height / 2),
  });
}
