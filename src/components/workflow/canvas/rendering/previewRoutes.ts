import {
  rectAnchor,
  routeEdge,
  RoutingObstacles,
  rectsIntersect,
  type FlowRect,
} from "../../../../flow";
import type { CanvasScene, SceneEdge } from "../../../../features/workflow";
import type { NodeDragPreview } from "../scene";

/** 一份指针预览最多推导一次；同帧多个移动只处理最后的位置。 */
const previews = new WeakMap<NodeDragPreview, readonly SceneEdge[]>();
export function previewRoutes(
  scene: CanvasScene,
  preview: NodeDragPreview,
): readonly SceneEdge[] {
  const cached = previews.get(preview);
  if (cached) return cached;
  const detail = scene.details[preview.scope],
    selected = new Set(preview.ids);
  const dirty: FlowRect[] = [];
  const rects = new Map(
    [...detail.rects].map(([id, rect]) => {
      if (!selected.has(id)) return [id, rect] as const;
      const moved = {
        ...rect,
        x: rect.x + preview.delta.x,
        y: rect.y + preview.delta.y,
      };
      for (const bounds of [rect, moved])
        dirty.push({
          x: bounds.x - 48,
          y: bounds.y - 48,
          width: bounds.width + 96,
          height: bounds.height + 96,
        });
      return [id, moved] as const;
    }),
  );
  const obstacles = new RoutingObstacles(
    [...rects].map(([id, rect]) => ({ ...rect, id })),
  );
  const edges = detail.edges.map((edge) => {
    if (
      !selected.has(edge.source) &&
      !selected.has(edge.target) &&
      !dirty.some((rect) => rectsIntersect(rect, edge.bounds))
    )
      return edge;
    const sourceAnchor = rectAnchor(
        rects.get(edge.source)!,
        edge.sourceAnchor.side,
        edge.source,
      ),
      targetAnchor = rectAnchor(
        rects.get(edge.target)!,
        edge.targetAnchor.side,
        edge.target,
      );
    return {
      ...edge,
      sourceAnchor,
      targetAnchor,
      ...routeEdge(sourceAnchor, targetAnchor, obstacles),
    };
  });
  previews.set(preview, edges);
  return edges;
}
