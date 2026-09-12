import { Crosshair, Expand, Hand, MousePointer2 } from "lucide-react";
import { compose, pointsBounds, transformRect } from "../../../flow";
import {
  studio,
  scopeElements,
  type CanvasScene,
  type EditorTab,
} from "../../../features/workflow";
import { IconButton } from "../../ui";
import type { CanvasToolMode } from "./useCanvasPointer";

/** 画布内的视图工具，上方工作流操作栏保持原有布局。 */
export function CanvasTools({
  tab,
  scene,
  size,
  mode,
  onModeChange,
  onFit,
}: {
  readonly tab: EditorTab;
  readonly scene: CanvasScene;
  readonly size: { readonly width: number; readonly height: number };
  readonly mode: CanvasToolMode;
  readonly onModeChange: (mode: CanvasToolMode) => void;
  readonly onFit: () => void;
}) {
  const locate = () => {
    const scope = scene.scopes[tab.scope] ?? scene.scopes[scene.root];
    const selected = scopeElements(scene, scope.id).filter((node) =>
      tab.selected.includes(node.id),
    );
    const bounds = selected.length
      ? pointsBounds(
          selected.flatMap((node) => {
            const rect = transformRect(node, scope.transform);
            return [rect, { x: rect.x + rect.width, y: rect.y + rect.height }];
          }),
        )
      : transformRect(scope.bounds, scope.transform);
    const center = compose(tab.viewport, {
      x: bounds.x + bounds.width / 2,
      y: bounds.y + bounds.height / 2,
      zoom: 1,
    });
    studio.view({
      ...tab.viewport,
      x: tab.viewport.x + size.width / 2 - center.x,
      y: tab.viewport.y + size.height / 2 - center.y,
    });
  };
  return (
    <div
      role="toolbar"
      aria-label="画布工具"
      className="absolute right-4 top-3 z-20 flex items-center gap-1 rounded-xl border border-line bg-surface/95 p-1 shadow-sm"
    >
      <IconButton
        aria-label="选择"
        aria-pressed={mode === "select"}
        className="aria-pressed:bg-accent-soft aria-pressed:text-accent"
        onClick={() => onModeChange("select")}
      >
        <MousePointer2 size={17} />
      </IconButton>
      <IconButton
        aria-label="平移"
        aria-pressed={mode === "pan"}
        className="aria-pressed:bg-accent-soft aria-pressed:text-accent"
        onClick={() => onModeChange("pan")}
      >
        <Hand size={17} />
      </IconButton>
      <span className="mx-1 h-5 border-l border-line" />
      <IconButton aria-label="居中显示" onClick={locate}>
        <Crosshair size={17} />
      </IconButton>
      <IconButton aria-label="显示全部" onClick={onFit}>
        <Expand size={17} />
      </IconButton>
    </div>
  );
}
