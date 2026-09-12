import {
  useCallback,
  useState,
  type DragEvent,
  type MouseEvent,
  type RefObject,
} from "react";
import {
  childScopes,
  nodeById,
  studio,
  type CanvasScene,
} from "../../../features/workflow";
import type { FlowPoint } from "../../../flow";
import { hitTest } from "./hitTest";
import type { CanvasLocation, NodePresentation } from "./scene";
import type { CanvasMenuPosition } from "./CanvasMenu";

/** 浮层只保存打开时的作用域与落点，不跟随相机和指针重新解释。 */
export function useCanvasActions(
  host: RefObject<HTMLDivElement | null>,
  scene: CanvasScene,
  nodes: ReadonlyMap<string, NodePresentation>,
  pointer: RefObject<CanvasLocation | null>,
  size: { readonly width: number; readonly height: number },
  focus: (scope: string) => void,
  pan: boolean,
) {
  const [search, setSearch] = useState<CanvasLocation | null>(null);
  const [menu, setMenu] = useState<CanvasMenuPosition | null>(null);
  const hit = (screen: FlowPoint) =>
    hitTest(scene, nodes, studio.active!.viewport, screen, {
      scope: studio.active!.scope,
      edge: studio.active!.selectedEdge,
    });
  const screen = (event: {
    readonly clientX: number;
    readonly clientY: number;
  }) => {
    const rect = host.current!.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };
  const location = () => {
    const saved = pointer.current;
    return saved && scene.scopes[saved.scope]
      ? saved
      : hit({ x: size.width / 2, y: size.height / 2 });
  };
  const add = () => {
    if (!studio.readonly) setSearch(location());
  };
  const close = useCallback(() => {
    setSearch(null);
    setMenu(null);
    host.current?.focus();
  }, [host]);
  const closeMenu = useCallback(() => {
    setMenu(null);
    host.current?.focus();
  }, [host]);
  const openMenu = (event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    const target = hit(screen(event));
    const tab = studio.active!;
    if (target.kind === "edge" || target.kind === "edge-end")
      studio.selectEdge(target.edge, target.scope);
    if (
      (target.kind === "node" || target.kind === "menu") &&
      (tab.scope !== target.scope || !tab.selected.includes(target.node))
    )
      studio.select([target.node], target.scope);
    else if (
      tab.scope !== target.scope &&
      target.kind !== "edge" &&
      target.kind !== "edge-end"
    )
      studio.select([], target.scope);
    setMenu({
      x: event.clientX,
      y: event.clientY,
      scope: target.scope,
      point: target.point,
      edge:
        target.kind === "edge" || target.kind === "edge-end"
          ? target.edge
          : undefined,
    });
  };
  const click = (event: MouseEvent<HTMLDivElement>) => {
    if (!pan && hit(screen(event)).kind === "menu") openMenu(event);
  };
  const doubleClick = (event: MouseEvent<HTMLDivElement>) => {
    if (pan) return;
    const target = hit(screen(event));
    if (target.kind === "node") {
      const node = nodeById(studio.active!.file, target.node);
      if (!node) return;
      const child = childScopes(node.action)[0];
      if (child) focus(child.id);
      else if (node.action.kind === "call_workflow") {
        const workflow = node.action.workflow;
        void studio.safely(() => studio.open(workflow));
      } else {
        studio.select([node.id], target.scope);
        document.querySelector<HTMLInputElement>("[data-node-title]")?.focus();
      }
    } else if (
      !studio.readonly &&
      (target.kind === "background" ||
        target.kind === "empty" ||
        target.kind === "edge")
    )
      setSearch(target);
  };
  const drop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    const kind = event.dataTransfer.getData("application/argusflow-node");
    if (!kind || studio.readonly) return;
    const target = hit(screen(event));
    void studio.safely(() =>
      studio.add(kind, target.point, target.connection, target.scope),
    );
  };
  const dragOver = (event: DragEvent<HTMLDivElement>) => {
    if (
      event.dataTransfer.types.includes("application/argusflow-node") &&
      !studio.readonly
    ) {
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
    }
  };
  return {
    search,
    setSearch,
    menu,
    location,
    add,
    close,
    closeMenu,
    openMenu,
    click,
    doubleClick,
    drop,
    dragOver,
  };
}
