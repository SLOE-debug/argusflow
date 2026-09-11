import {
  alignNodes,
  distributeNodes,
  type AlignMode,
  type DistributeMode,
} from "../../../flow";
import { buildScene } from "../model/layout";
import { scopeById, setLayout } from "../model/graph";
import { studio } from "./controller";

export type Arrangement = AlignMode | DistributeMode | "tidy";
/** 工具栏、属性面板和右键菜单共享排列事务。 */
export function arrangeSelection(mode: Arrangement): void {
  const tab = studio.active;
  if (!tab || studio.readonly) return;
  const active = buildScene(tab.file).scopes[tab.scope];
  if (mode === "tidy") {
    const scope = scopeById(tab.file, tab.scope);
    let id = scope.entry,
      x = 80;
    const visited = new Set<string>();
    studio.edit((file) => {
      let next = file;
      while (id && !visited.has(id)) {
        visited.add(id);
        next = setLayout(next, id, { x, y: 100 });
        x += (active.nodes.find((node) => node.id === id)?.width ?? 196) + 80;
        id = scope.nodes.find((node) => node.id === id)?.next ?? null;
      }
      let y = 240;
      for (const node of active.nodes)
        if (!visited.has(node.id)) {
          next = setLayout(next, node.id, { x: 80, y });
          y += node.height + 48;
        }
      return next;
    });
    return;
  }
  const nodes = active.nodes.map((rect) => ({
    id: rect.id,
    position: { x: rect.x, y: rect.y },
    size: { width: rect.width, height: rect.height },
    data: null,
  }));
  const selected = new Set(tab.selected);
  const result =
    mode === "horizontal" || mode === "vertical"
      ? distributeNodes(nodes, selected, mode)
      : alignNodes(nodes, selected, mode);
  studio.edit((file) =>
    result.reduce(
      (next, node) => setLayout(next, node.id, node.position),
      file,
    ),
  );
}
