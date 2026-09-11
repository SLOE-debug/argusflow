import { Crosshair, Expand, Hand, Map, MousePointer2 } from "lucide-react";
import { fitBounds, pointsBounds } from "../../../flow";
import {
  studio,
  type EditorTab,
  type ScopeGeometry,
} from "../../../features/workflow";
import { IconButton } from "../../ui";
import type { CanvasToolMode } from "./useCanvasPointer";

/** 常驻右上角的画布工具，视口操作不修改工作流内容。 */
export function CanvasTools({
  tab,
  scope,
  size,
  mode,
  onModeChange,
  minimap,
  onMinimapChange,
}: {
  readonly tab: EditorTab;
  readonly scope: ScopeGeometry;
  readonly size: { readonly width: number; readonly height: number };
  readonly mode: CanvasToolMode;
  readonly onModeChange: (mode: CanvasToolMode) => void;
  readonly minimap: boolean;
  readonly onMinimapChange: () => void;
}) {
  /** 优先定位选中节点，没有选择时定位全部节点；保留当前缩放。 */
  const locate = () => {
    const selected = scope.nodes.filter((node) =>
      tab.selected.includes(node.id),
    );
    const nodes = selected.length ? selected : scope.nodes;
    if (!nodes.length) return;
    const bounds = pointsBounds(
      nodes.flatMap((node) => [
        { x: node.x, y: node.y },
        { x: node.x + node.width, y: node.y + node.height },
      ]),
    );
    studio.view({
      ...tab.viewport,
      x: size.width / 2 - (bounds.x + bounds.width / 2) * tab.viewport.zoom,
      y: size.height / 2 - (bounds.y + bounds.height / 2) * tab.viewport.zoom,
    });
  };
  return (
    <div
      data-canvas-tool
      role="toolbar"
      aria-label="画布工具"
      className="absolute right-4 top-3 z-20 flex items-center gap-2"
      onPointerDown={(event) => event.stopPropagation()}
    >
      <div className="flex overflow-hidden rounded-md border border-line bg-surface shadow-sm [&>button]:size-8 [&>button]:rounded-none [&>button]:border-0 [&>button+button]:border-l [&>button+button]:border-line">
        <IconButton
          aria-label="选择"
          aria-pressed={mode === "select"}
          className="aria-pressed:bg-accent-soft aria-pressed:text-accent"
          onClick={() => onModeChange("select")}
        >
          <MousePointer2 size={15} />
        </IconButton>
        <IconButton
          aria-label="平移"
          aria-pressed={mode === "pan"}
          className="aria-pressed:bg-accent-soft aria-pressed:text-accent"
          onClick={() => onModeChange("pan")}
        >
          <Hand size={15} />
        </IconButton>
      </div>
      <div className="flex overflow-hidden rounded-md border border-line bg-surface shadow-sm [&>button]:size-8 [&>button]:rounded-none [&>button]:border-0 [&>button+button]:border-l [&>button+button]:border-line">
        <IconButton
          aria-label="居中显示"
          disabled={!scope.nodes.length}
          onClick={locate}
        >
          <Crosshair size={15} />
        </IconButton>
        <IconButton
          aria-label="显示全部"
          disabled={!scope.nodes.length}
          onClick={() =>
            studio.view(fitBounds(scope.bounds, size.width, size.height))
          }
        >
          <Expand size={15} />
        </IconButton>
        <IconButton
          aria-label="小地图"
          aria-pressed={minimap}
          className="aria-pressed:bg-accent-soft aria-pressed:text-accent"
          onClick={onMinimapChange}
        >
          <Map size={15} />
        </IconButton>
      </div>
    </div>
  );
}
