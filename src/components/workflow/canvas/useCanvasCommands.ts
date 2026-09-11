import { useEffect, type RefObject } from "react";
import {
  hasTextSelection,
  ownsKeyboard,
  screenToWorld,
  type FlowPoint,
} from "../../../flow";
import {
  studio,
  scopeById,
  setLayout,
  childScopes,
  nodeById,
} from "../../../features/workflow";
export function useCanvasCommands(
  host: RefObject<HTMLDivElement | null>,
  pointer: RefObject<FlowPoint | null>,
  add: () => void,
  activate: (scope: string) => void,
  cancel: () => void,
) {
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const key = (event: KeyboardEvent) => {
      if (ownsKeyboard(event)) return;
      const tab = studio.active;
      if (!tab) return;
      const modifier = event.ctrlKey || event.metaKey;
      const letter = event.key.toLowerCase();
      const center =
        pointer.current ??
        screenToWorld(
          { x: element.clientWidth / 2, y: element.clientHeight / 2 },
          tab.viewport,
        );
      const run = (action: () => unknown) => {
        event.preventDefault();
        void studio.safely(action);
      };
      if (modifier && letter === "c" && !hasTextSelection())
        return run(() => studio.copy());
      if (modifier && letter === "a")
        return run(() =>
          studio.select(
            scopeById(tab.file, tab.scope).nodes.map((node) => node.id),
          ),
        );
      if (event.key === "Escape") return run(cancel);
      if (studio.readonly) return;
      if (modifier && letter === "x" && !hasTextSelection())
        return run(() => studio.copy(true));
      if (modifier && letter === "v") return run(() => studio.paste(center));
      if (modifier && letter === "d") return run(() => studio.duplicate());
      if (modifier && letter === "z")
        return run(() => (event.shiftKey ? studio.redo() : studio.undo()));
      if (modifier && letter === "y") return run(() => studio.redo());
      if (event.key === "Delete" || event.key === "Backspace")
        return run(() => studio.remove());
      if (event.key === "Tab") return run(add);
      if (event.key === "F2")
        return run(() =>
          document
            .querySelector<HTMLInputElement>("[data-node-title]")
            ?.focus(),
        );
      if (event.key === "Enter" && tab.selected.length === 1) {
        const node = nodeById(tab.file, tab.selected[0]);
        if (!node) return;
        const child = childScopes(node.action)[0];
        return run(() =>
          child
            ? activate(child.id)
            : node.action.kind === "call_workflow"
              ? studio.open(node.action.workflow)
              : document
                  .querySelector<HTMLInputElement>("[data-node-title]")
                  ?.focus(),
        );
      }
      const directions: Record<string, FlowPoint> = {
        ArrowLeft: { x: -1, y: 0 },
        ArrowRight: { x: 1, y: 0 },
        ArrowUp: { x: 0, y: -1 },
        ArrowDown: { x: 0, y: 1 },
      };
      const direction = directions[event.key];
      if (!modifier && direction && tab.selected.length)
        run(() =>
          studio.edit(
            (file) =>
              tab.selected.reduce((next, id) => {
                const layout = file.editor.nodes[id];
                const distance = event.shiftKey ? 10 : 1;
                return setLayout(next, id, {
                  x: layout.x + direction.x * distance,
                  y: layout.y + direction.y * distance,
                });
              }, file),
            !event.repeat,
          ),
        );
    };
    element.addEventListener("keydown", key);
    return () => element.removeEventListener("keydown", key);
  }, [host, pointer, add, activate, cancel]);
}
