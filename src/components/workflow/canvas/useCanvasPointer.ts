import { useRef, useState, type PointerEvent, type RefObject } from "react";
import {
  rectFromPoints,
  rectsIntersect,
  screenToWorld,
  type FlowPoint,
  type FlowRect,
  type ViewportTransform,
} from "../../../flow";
import {
  connectNodes,
  setLayout,
  studio,
  type Scene,
  type WorkflowFile,
} from "../../../features/workflow";

type Gesture =
  | {
      readonly kind: "pan";
      readonly start: FlowPoint;
      readonly view: ViewportTransform;
    }
  | {
      readonly kind: "box";
      readonly start: FlowPoint;
      readonly initial: readonly string[];
    }
  | {
      readonly kind: "nodes";
      readonly start: FlowPoint;
      readonly file: WorkflowFile;
      readonly ids: readonly string[];
    }
  | { readonly kind: "wire"; readonly from: string; readonly start: FlowPoint };
/** 指针的常驻工具；空格和鼠标中键可临时启用平移。 */
export type CanvasToolMode = "select" | "pan";
/** 指针手势只有一个所有者，移动批次只产生一次历史记录。 */
export function useCanvasPointer(
  host: RefObject<HTMLDivElement | null>,
  scene: Scene,
  addConnected: (point: FlowPoint, source: string) => void,
  mode: CanvasToolMode,
) {
  const gesture = useRef<Gesture | null>(null);
  const pointer = useRef<FlowPoint | null>(null);
  const space = useRef(false);
  const [box, setBox] = useState<FlowRect | null>(null);
  const [wire, setWire] = useState<{
    readonly from: FlowPoint;
    readonly to: FlowPoint;
  } | null>(null);
  const point = (event: {
    readonly clientX: number;
    readonly clientY: number;
  }): FlowPoint => {
    const rect = host.current!.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };
  const cancel = () => {
    const active = gesture.current;
    if (active?.kind === "nodes" && studio.active?.past.at(-1) === active.file)
      studio.undo();
    if (active?.kind === "pan") studio.view(active.view);
    gesture.current = null;
    setBox(null);
    setWire(null);
    space.current = false;
  };
  const down = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button === 2) return;
    const tab = studio.active;
    if (!tab) return;
    const target = event.target as Element;
    if (target.closest("[data-canvas-tool]")) return;
    host.current?.focus();
    const screen = point(event),
      world = screenToWorld(screen, tab.viewport);
    pointer.current = world;
    if (event.button === 1 || space.current || mode === "pan") {
      event.preventDefault();
      gesture.current = { kind: "pan", start: screen, view: tab.viewport };
    } else if (!studio.readonly) {
      const port = target.closest<HTMLElement>("[data-port]");
      const nodeElement = target.closest<HTMLElement>("[data-node]");
      if (port?.dataset.port === "out") {
        const from = port.dataset.portNode!;
        gesture.current = { kind: "wire", from, start: world };
        setWire({ from: screen, to: screen });
      } else if (nodeElement?.dataset.nodeScope === tab.scope) {
        const id = nodeElement.dataset.node!;
        const ids = event.shiftKey
          ? tab.selected.includes(id)
            ? tab.selected.filter((item) => item !== id)
            : [...tab.selected, id]
          : tab.selected.includes(id)
            ? tab.selected
            : [id];
        studio.select(ids);
        gesture.current = { kind: "nodes", start: world, file: tab.file, ids };
      } else if (!nodeElement) {
        gesture.current = {
          kind: "box",
          start: world,
          initial: event.shiftKey ? tab.selected : [],
        };
        if (!event.shiftKey) studio.select([]);
      }
    } else {
      const nodeElement = target.closest<HTMLElement>("[data-node]");
      if (nodeElement?.dataset.nodeScope === tab.scope)
        studio.select([nodeElement.dataset.node!]);
    }
    if (gesture.current) event.currentTarget.setPointerCapture(event.pointerId);
  };
  const move = (event: PointerEvent<HTMLDivElement>) => {
    const tab = studio.active;
    if (!tab) return;
    const screen = point(event),
      world = screenToWorld(screen, tab.viewport);
    pointer.current = world;
    const active = gesture.current;
    if (!active) return;
    if (active.kind === "pan")
      studio.view({
        ...active.view,
        x: active.view.x + screen.x - active.start.x,
        y: active.view.y + screen.y - active.start.y,
      });
    if (active.kind === "box") {
      const bounds = rectFromPoints(active.start, world);
      setBox(bounds);
      studio.select([
        ...new Set([
          ...active.initial,
          ...scene.scopes[tab.scope].nodes
            .filter((node) => rectsIntersect(bounds, node))
            .map((node) => node.id),
        ]),
      ]);
    }
    if (active.kind === "wire")
      setWire({
        from: {
          x: active.start.x * tab.viewport.zoom + tab.viewport.x,
          y: active.start.y * tab.viewport.zoom + tab.viewport.y,
        },
        to: screen,
      });
    if (active.kind === "nodes") {
      const delta = {
        x: Math.round((world.x - active.start.x) / 8) * 8,
        y: Math.round((world.y - active.start.y) / 8) * 8,
      };
      if (!delta.x && !delta.y && tab.file === active.file) return;
      studio.edit(
        (file) =>
          active.ids.reduce((next, id) => {
            const origin = active.file.editor.nodes[id];
            return setLayout(next, id, {
              x: origin.x + delta.x,
              y: origin.y + delta.y,
            });
          }, file),
        fileUnchanged(tab.file, active.file),
      );
    }
  };
  const up = (event: PointerEvent<HTMLDivElement>) => {
    const active = gesture.current,
      tab = studio.active;
    if (active?.kind === "wire" && tab) {
      const target = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>('[data-port="in"]');
      if (target?.dataset.portNode)
        void studio.safely(() =>
          studio.edit((file) =>
            connectNodes(
              file,
              tab.scope,
              active.from,
              target.dataset.portNode === "$exit"
                ? null
                : target.dataset.portNode!,
            ),
          ),
        );
      else addConnected(screenToWorld(point(event), tab.viewport), active.from);
    }
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
    gesture.current = null;
    setBox(null);
    setWire(null);
  };
  return { down, move, up, cancel, pointer, box, wire, space };
}
function fileUnchanged(current: WorkflowFile, initial: WorkflowFile): boolean {
  return current === initial;
}
