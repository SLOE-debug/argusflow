import {
  alignNodes,
  distributeNodes,
  type AlignMode,
  type DistributeMode,
} from "../../../flow";
import { buildCanvasScene, scopeElements } from "../model/canvas-scene";
import { setLayout } from "../model/graph";
import { tidyScope } from "./tidy";
import { studio } from "./controller";

export type Arrangement = AlignMode | DistributeMode | "tidy";
/** 工具栏、属性面板和右键菜单共享排列事务。 */
export function arrangeSelection(mode: Arrangement): void {
  const tab = studio.active;
  if (!tab || studio.readonly) return;
  const scene = buildCanvasScene(tab.file);
  if (mode === "tidy") {
    studio.edit((file) => tidyScope(file, tab.scope));
    return;
  }
  const nodes = scopeElements(scene, tab.scope).map((rect) => ({
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
