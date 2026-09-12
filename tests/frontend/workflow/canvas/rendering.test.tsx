import { act, cleanup } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  createNode,
  createWorkflow,
  setLayout,
  studio,
  type WorkflowFile,
} from "../../../../src/features/workflow";
import { fitBounds, transformRect } from "../../../../src/flow";
import { addNode } from "../../../../src/features/workflow/model/graph";
import { activeTheme, setTheme } from "../../../../src/features/themes";
import { presentNodes } from "../../../../src/components/workflow/canvas/scene";
import {
  drawScene,
  type SceneFrame,
} from "../../../../src/components/workflow/canvas/rendering/drawScene";
import { DrawingResources } from "../../../../src/components/workflow/canvas/rendering/resources";
import { recordingContext } from "../../support/canvas";
import {
  canvasEnvironment,
  installCanvas,
  nestedFixture,
} from "../../support/canvasFixture";

beforeEach(() => {
  vi.useFakeTimers();
  canvasEnvironment();
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  setTheme("light");
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function frameFor(file: WorkflowFile): SceneFrame {
  return {
    scene: buildCanvasScene(file),
    nodes: presentNodes(file),
    camera: { x: 40, y: 30, zoom: 1 },
    width: 1200,
    height: 800,
    colors: activeTheme().colors,
    selected: [],
    selectedEdge: null,
    readonly: false,
    hover: null,
    scope: file.definition.root,
    states: new Map(),
    runningDocument: false,
    preview: null,
    box: null,
    wire: null,
  };
}

it("节点、连线和起止标记实际发出绘制调用，文字测量在平移时复用", () => {
  const fixture = nestedFixture();
  const { calls, context } = recordingContext();
  const resources = new DrawingResources(vi.fn());
  const start = addNode(fixture.file, fixture.root, "start", { x: 80, y: 300 });
  const end = addNode(start.file, fixture.root, "end", { x: 500, y: 300 });
  const file = setLayout(
    setLayout(end.file, start.id, { x: 80, y: 300 }),
    end.id,
    { x: 500, y: 300 },
  );
  const frame = frameFor(file);
  expect(drawScene(context, resources, frame)).toBe(4);
  expect(calls.fillText.mock.calls.map(([text]) => text)).toContain("开始");
  expect(calls.fillText.mock.calls.map(([text]) => text)).toContain("结束");
  expect(calls.fillText.mock.calls.map(([text]) => text)).toContain("循环体");
  expect(calls.lineTo).toHaveBeenCalled();
  expect(calls.roundRect).toHaveBeenCalledWith(80, 100, 232, 80, 12);
  const measured = calls.measureText.mock.calls.length;
  drawScene(context, resources, {
    ...frame,
    camera: { x: 60, y: 40, zoom: 1 },
  });
  expect(calls.measureText).toHaveBeenCalledTimes(measured);
  expect(buildCanvasScene(file)).toBe(frame.scene);
  expect(calls.save.mock.calls.length).toBe(calls.restore.mock.calls.length);
  calls.roundRect.mockClear();
  calls.arc.mockClear();
  drawScene(context, resources, {
    ...frame,
    selected: [start.id],
    hover: { kind: "node", scope: fixture.root, id: start.id },
    preview: {
      scope: fixture.root,
      ids: [start.id],
      delta: { x: 40, y: -24 },
      guides: [],
    },
  });
  expect(calls.roundRect).toHaveBeenCalledWith(120, 276, 168, 72, 12);
  expect(calls.arc).toHaveBeenCalledWith(288, 312, 4, 0, Math.PI * 2);
  resources.dispose();
});

it("一千节点仅绘制视口附近，缩放和平移复用派生场景与路径", () => {
  const empty = createWorkflow();
  const nodes = Array.from({ length: 1000 }, () => createNode("wait").node);
  const file: WorkflowFile = {
    ...empty,
    definition: {
      ...empty.definition,
      scopes: [
        {
          ...empty.definition.scopes[0],
          edges: nodes.slice(0, -1).map((node, index) => ({
            id: "edge_" + index,
            source: { kind: "node" as const, node: node.id },
            target: { kind: "node" as const, node: nodes[index + 1].id },
          })),
          nodes,
        },
      ],
    },
    editor: {
      drafts: {},
      edges: Object.fromEntries(
        nodes
          .slice(0, -1)
          .map((_, index) => [
            "edge_" + index,
            { source: "right" as const, target: "left" as const },
          ]),
      ),
      nodes: Object.fromEntries(
        nodes.map((node, index) => [
          node.id,
          { x: index * 320, y: 100, label: "等待 " + index, note: "" },
        ]),
      ),
    },
  };
  const frame = frameFor(file);
  const { calls, context } = recordingContext();
  const resources = new DrawingResources(vi.fn());
  const count = drawScene(context, resources, frame);
  expect(count).toBeGreaterThan(0);
  expect(count).toBeLessThan(10);
  expect(frame.scene.details[frame.scene.root].edges).toHaveLength(999);
  const paths = frame.scene.details[frame.scene.root].edges;
  const next = { ...frame, camera: { x: -160000, y: 0, zoom: 1 } };
  expect(drawScene(context, resources, next)).toBeLessThan(10);
  expect(buildCanvasScene(file).details[frame.scene.root].edges).toBe(paths);
  const bounds = frame.scene.scopes[frame.scene.root].bounds;
  const fitted = transformRect(bounds, fitBounds(bounds, 1200, 800));
  expect(fitted.x).toBeGreaterThanOrEqual(49);
  expect(fitted.x + fitted.width).toBeLessThanOrEqual(1151);
  expect(calls.roundRect.mock.calls.length).toBeLessThan(30);
  resources.dispose();
});

it("帧请求合并、静止不重绘，卸载取消待绘制回调", () => {
  const recording = canvasEnvironment();
  const fixture = nestedFixture();
  const { unmount } = installCanvas(fixture.file);
  act(() => vi.advanceTimersByTime(32));
  const count = recording.calls.clearRect.mock.calls.length;
  expect(count).toBe(1);
  act(() => {
    studio.view({ x: 100, y: 50, zoom: 1 });
    studio.view({ x: 120, y: 50, zoom: 1 });
    studio.view({ x: 140, y: 50, zoom: 1 });
  });
  act(() => vi.advanceTimersByTime(32));
  expect(recording.calls.clearRect).toHaveBeenCalledTimes(count + 1);
  act(() => vi.advanceTimersByTime(200));
  expect(recording.calls.clearRect).toHaveBeenCalledTimes(count + 1);
  act(() => studio.view({ x: 160, y: 50, zoom: 1 }));
  unmount();
  act(() => vi.advanceTimersByTime(32));
  expect(recording.calls.clearRect).toHaveBeenCalledTimes(count + 1);
});

it("设备像素比、字体和主题变化触发重绘并清理订阅", () => {
  const recording = canvasEnvironment();
  const fonts = new EventTarget();
  vi.stubGlobal("devicePixelRatio", 2);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: fonts,
  });
  const remove = vi.spyOn(fonts, "removeEventListener");
  const fixture = nestedFixture();
  const { host, unmount } = installCanvas(fixture.file);
  act(() => vi.advanceTimersByTime(32));
  expect(host.querySelector("canvas")!.width).toBe(2400);
  expect(host.querySelector("canvas")!.height).toBe(1600);
  expect(recording.calls.setTransform).toHaveBeenCalledWith(2, 0, 0, 2, 0, 0);
  const measured = recording.calls.measureText.mock.calls.length;
  act(() => fonts.dispatchEvent(new Event("loadingdone")));
  act(() => vi.advanceTimersByTime(32));
  expect(recording.calls.measureText.mock.calls.length).toBeGreaterThan(
    measured,
  );
  act(() => setTheme("dark"));
  act(() => vi.advanceTimersByTime(32));
  expect(recording.calls.fillStyle).not.toBe("");
  expect(recording.calls.clearRect).toHaveBeenCalledTimes(3);
  vi.stubGlobal("devicePixelRatio", 1.5);
  act(() => window.dispatchEvent(new Event("resize")));
  act(() => vi.advanceTimersByTime(32));
  expect(host.querySelector("canvas")!.width).toBe(1800);
  unmount();
  expect(remove).toHaveBeenCalledWith("loadingdone", expect.any(Function));
  Reflect.deleteProperty(document, "fonts");
});

it("图标缓存生成有效 SVG，卸载后不再触发重绘", () => {
  const images: HTMLImageElement[] = [];
  vi.stubGlobal(
    "Image",
    class {
      onload: (() => void) | null = null;
      src = "";
      complete = true;
      naturalWidth = 24;
      constructor() {
        images.push(this as unknown as HTMLImageElement);
      }
    },
  );
  const invalidate = vi.fn();
  const resources = new DrawingResources(invalidate);
  const { context, calls } = recordingContext();
  resources.icon(context, "while", "#112233", 0, 0);
  resources.icon(context, "while", "#112233", 10, 10);
  expect(images).toHaveLength(1);
  const svg = decodeURIComponent(images[0].src.split(",")[1]);
  const parsed = new DOMParser().parseFromString(svg, "image/svg+xml");
  expect(parsed.querySelector("parsererror")).toBeNull();
  expect(parsed.documentElement.getAttribute("color")).toBe("#112233");
  expect(calls.drawImage).toHaveBeenCalledTimes(2);
  resources.dispose();
  expect(images[0].onload).toBeNull();
});
