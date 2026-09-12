import {
  useEffect,
  useRef,
  useState,
  type PointerEvent,
  type RefObject,
} from "react";
import {
  compose,
  rectFromPoints,
  rectsIntersect,
  transformRect,
  type FlowPoint,
  type FlowRect,
  type ViewportTransform,
} from "../../../flow";
import {
  createConnection,
  reconnectEdge,
  scopeElements,
  studio,
  type CanvasScene,
} from "../../../features/workflow";
import { hitTest } from "./hitTest";
import { beginWire, previewWire, type WireGesture } from "./wireGesture";
import { NodeDragGesture } from "./nodeDragGesture";
import type { InteractionPreview } from "./rendering/interactionPreview";
import type { CanvasLocation, NodePresentation } from "./scene";

/** 空格和中键可临时启用平移。 */
export type CanvasToolMode = "select" | "pan";
type Gesture =
  | {
      readonly kind: "pan";
      readonly document: string;
      readonly start: FlowPoint;
      readonly view: ViewportTransform;
    }
  | {
      readonly kind: "box";
      readonly start: FlowPoint;
      readonly scope: string;
      readonly initial: readonly string[];
    }
  | NodeDragGesture
  | WireGesture;

/** 指针手势拥有临时预览，松手提交，取消不污染历史。 */
export function useCanvasPointer(
  host: RefObject<HTMLDivElement | null>,
  scene: CanvasScene,
  nodes: ReadonlyMap<string, NodePresentation>,
  addConnected: (location: CanvasLocation) => void,
  mode: CanvasToolMode,
  stopNavigation: () => void,
  preview: InteractionPreview,
) {
  const gesture = useRef<Gesture | null>(null);
  const pointer = useRef<CanvasLocation | null>(null);
  const space = useRef(false);
  const capture = useRef<number | null>(null);
  const [box, setBox] = useState<FlowRect | null>(null);
  const suppressClick = useRef(false);
  const origin = useRef<FlowPoint | null>(null);
  const [cursor, setCursor] = useState("");
  const screenPoint = (event: {
    readonly clientX: number;
    readonly clientY: number;
  }): FlowPoint => {
    const rect = origin.current ?? host.current!.getBoundingClientRect();
    return { x: event.clientX - rect.x, y: event.clientY - rect.y };
  };
  const release = () => {
    const element = host.current;
    if (capture.current !== null && element?.hasPointerCapture(capture.current))
      element.releasePointerCapture(capture.current);
    capture.current = null;
  };
  const clear = () => {
    gesture.current = null;
    origin.current = null;
    if (host.current) host.current.style.cursor = "";
    preview.clear();
    setBox(null);
    release();
  };
  const cancel = () => {
    const active = gesture.current;
    if (active?.kind === "box") studio.select(active.initial, active.scope);
    clear();
    space.current = false;
  };
  const cancelOnBlur = useRef(cancel);
  cancelOnBlur.current = cancel;
  useEffect(() => {
    const blur = () => cancelOnBlur.current();
    window.addEventListener("blur", blur);
    return () => window.removeEventListener("blur", blur);
  }, []);
  useEffect(
    () => () => {
      const element = host.current;
      if (
        capture.current !== null &&
        element?.hasPointerCapture(capture.current)
      )
        element.releasePointerCapture(capture.current);
    },
    [host],
  );
  useEffect(() => {
    if (gesture.current && gesture.current.kind !== "pan") clear();
    preview.clear();
  }, [scene]);
  const down = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button === 2 || gesture.current) return;
    const tab = studio.active;
    if (!tab) return;
    stopNavigation();
    host.current?.focus();
    if (event.button === 1 || space.current || mode === "pan") {
      event.preventDefault();
      // 平移只需客户端坐标差，不做命中检测或逐事件读取 DOM 布局。
      pointer.current = null;
      gesture.current = {
        kind: "pan",
        document: tab.file.id,
        start: { x: event.clientX, y: event.clientY },
        view: tab.viewport,
      };
      capture.current = event.pointerId;
      event.currentTarget.setPointerCapture(event.pointerId);
      return;
    }
    const screen = screenPoint(event);
    origin.current = {
      x: event.clientX - screen.x,
      y: event.clientY - screen.y,
    };
    const hit = hitTest(scene, nodes, tab.viewport, screen, {
      scope: tab.scope,
      edge: tab.selectedEdge,
    });
    suppressClick.current = false;
    pointer.current = hit;
    if (hit.kind === "node") {
      const previous = tab.scope === hit.scope ? tab.selected : [];
      const ids = event.shiftKey
        ? previous.includes(hit.node)
          ? previous.filter((id) => id !== hit.node)
          : [...previous, hit.node]
        : previous.includes(hit.node)
          ? previous
          : [hit.node];
      studio.select(ids, hit.scope);
      if (!studio.readonly && ids.length)
        gesture.current = new NodeDragGesture(
          scene,
          tab.file,
          hit.scope,
          ids,
          { x: event.clientX, y: event.clientY },
          compose(tab.viewport, scene.scopes[hit.scope].transform),
        );
    } else if (hit.kind === "edge" || hit.kind === "edge-end") {
      studio.selectEdge(hit.edge, hit.scope);
      if (!studio.readonly && hit.kind === "edge-end") {
        gesture.current = beginWire(hit, scene, tab.file);
        suppressClick.current = true;
      }
    } else if (!studio.readonly && hit.kind === "port" && hit.port === "out") {
      studio.select([], hit.scope);
      gesture.current = beginWire(hit, scene, tab.file);
      suppressClick.current = true;
    } else if (
      !studio.readonly &&
      (hit.kind === "background" || hit.kind === "empty")
    ) {
      const initial =
        event.shiftKey && tab.scope === hit.scope ? tab.selected : [];
      studio.select(initial, hit.scope);
      gesture.current = {
        kind: "box",
        start: screen,
        scope: hit.scope,
        initial,
      };
    }
    if (gesture.current) {
      if (gesture.current.kind === "wire") {
        const active = gesture.current;
        preview.updateInteraction(scene, null, {
          scope: active.scope,
          source: active.source,
          target: active.target,
          replacing: active.edge,
          candidate: null,
          error: null,
        });
      }
      capture.current = event.pointerId;
      event.currentTarget.setPointerCapture(event.pointerId);
    }
  };
  const panPosition = (
    active: Extract<Gesture, { kind: "pan" }>,
    event: {
      readonly clientX: number;
      readonly clientY: number;
    },
  ): ViewportTransform => ({
    ...active.view,
    x: active.view.x + event.clientX - active.start.x,
    y: active.view.y + event.clientY - active.start.y,
  });
  const move = (event: PointerEvent<HTMLDivElement>) => {
    const tab = studio.active;
    if (!tab) return;
    const active = gesture.current;
    if (active?.kind === "pan") {
      if (tab.file.id !== active.document || tab.viewport !== active.view) {
        clear();
        return;
      }
      preview.update(active.view, panPosition(active, event));
      return;
    }
    if (active?.kind === "nodes") {
      if (studio.readonly || tab.file !== active.file) {
        clear();
        return;
      }
      preview.updateNodes(
        scene,
        active.update({ x: event.clientX, y: event.clientY }, event.altKey),
      );
      return;
    }
    const screen = screenPoint(event);
    if (!active) {
      const hit = hitTest(scene, nodes, tab.viewport, screen, {
        scope: tab.scope,
        edge: tab.selectedEdge,
      });
      pointer.current = hit;
      preview.updateInteraction(
        scene,
        hit.kind === "node" || hit.kind === "port" || hit.kind === "menu"
          ? { kind: "node", id: hit.node, scope: hit.scope }
          : hit.kind === "edge" || hit.kind === "edge-end"
            ? { kind: "edge", id: hit.edge, scope: hit.scope }
            : null,
      );
      setCursor(
        hit.kind === "port"
          ? hit.port === "out" && !studio.readonly
            ? "cursor-crosshair"
            : "cursor-default"
          : hit.kind === "edge-end"
            ? "cursor-grab"
            : hit.kind === "edge"
              ? "cursor-pointer"
              : hit.kind === "node"
                ? "cursor-move"
                : hit.kind === "menu" || hit.kind === "empty"
                  ? "cursor-pointer"
                  : "",
      );
      return;
    }
    if (active.kind === "box") {
      const bounds = rectFromPoints(active.start, screen);
      setBox(bounds);
      const scope = scene.scopes[active.scope];
      const view = compose(tab.viewport, scope.transform);
      studio.select(
        [
          ...new Set([
            ...active.initial,
            ...scopeElements(scene, active.scope)
              .filter((node) =>
                rectsIntersect(bounds, transformRect(node, view)),
              )
              .map((node) => node.id),
          ]),
        ],
        active.scope,
      );
    } else if (active.kind === "wire") {
      if (studio.readonly || tab.file !== active.file) {
        clear();
        return;
      }
      const wire = previewWire(
        active,
        scene,
        tab.viewport,
        screen,
        hitTest(scene, nodes, tab.viewport, screen),
      );
      preview.updateInteraction(scene, null, wire);
      const cursor = wire.error ? "not-allowed" : "crosshair";
      if (host.current && host.current.style.cursor !== cursor)
        host.current.style.cursor = cursor;
    }
  };
  const up = (event: PointerEvent<HTMLDivElement>) => {
    const active = gesture.current,
      tab = studio.active;
    if (active?.kind === "pan") {
      clear();
      if (tab?.file.id === active.document && tab.viewport === active.view) {
        const next = panPosition(active, event);
        if (next.x !== active.view.x || next.y !== active.view.y)
          studio.view(next);
      }
      return;
    }
    if (active && tab && !studio.readonly) {
      if (active.kind === "nodes") {
        active.commit({ x: event.clientX, y: event.clientY }, event.altKey);
      } else if (active.kind === "wire" && tab.file === active.file) {
        const screen = screenPoint(event);
        const hit = hitTest(scene, nodes, tab.viewport, screen);
        const wire = previewWire(active, scene, tab.viewport, screen, hit);
        if (wire.candidate && !wire.error) {
          void studio.safely(() => {
            studio.edit((file) =>
              active.edge
                ? reconnectEdge(
                    file,
                    active.scope,
                    active.edge,
                    active.moving,
                    wire.candidate!,
                    wire[active.moving].side,
                  )
                : createConnection(
                    file,
                    active.scope,
                    wire.source.owner!,
                    wire.target.owner!,
                    { source: wire.source.side, target: wire.target.side },
                  ),
            );
            const id =
              active.edge ??
              studio
                .active!.file.definition.scopes.find(
                  (scope) => scope.id === active.scope,
                )!
                .edges.at(-1)!.id;
            studio.selectEdge(id, active.scope);
          });
        } else if (wire.error) studio.message(wire.error);
        else if (
          !active.edge &&
          hit.scope === active.scope &&
          (hit.kind === "background" || hit.kind === "empty")
        ) {
          addConnected({
            scope: active.scope,
            point: hit.point,
            connection: {
              kind: "from",
              node: active.source.owner!,
              side: active.source.side,
            },
          });
        }
      }
    }
    clear();
  };
  return {
    down,
    move,
    up,
    cancel,
    pointer,
    box,
    snapModifierChanged: (disabled: boolean) => {
      const active = gesture.current;
      if (active?.kind !== "nodes") return false;
      if (studio.readonly || studio.active?.file !== active.file) clear();
      else preview.updateNodes(scene, active.update(active.point, disabled));
      return true;
    },
    leave: () => {
      if (!gesture.current) {
        preview.clear();
        setCursor("");
      }
    },
    consumeClick: () => {
      const suppressed = suppressClick.current;
      suppressClick.current = false;
      return suppressed;
    },
    space,
    cursor,
    screenPoint,
  };
}
