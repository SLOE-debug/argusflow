import {
  compose,
  isRectVisible,
  rectAnchor,
  ANCHOR_SIDES,
  type FlowPoint,
  type FlowRect,
  type ViewportTransform,
} from "../../../../flow";
import type { CanvasScene } from "../../../../features/workflow";
import type { ThemeDefinition } from "../../../../features/themes";
import type { NodeRunState } from "../../execution/nodeStates";
import type {
  NodeDragPreview,
  NodePresentation,
  CanvasHover,
  WirePreview,
} from "../scene";
import { drawCard, drawEndpoint, drawPort } from "./cards";
import type { DrawingResources } from "./resources";
import { drawBackground } from "./background";
import { drawEdge, drawEdgeHandle, wireRoute } from "./edges";
import { previewRoutes } from "./previewRoutes";
import { drawSnapGuides } from "./guides";

/** 绘制帧输入只读，几何与展示数据由宿主缓存。 */
export interface SceneFrame {
  readonly scene: CanvasScene;
  readonly nodes: ReadonlyMap<string, NodePresentation>;
  readonly camera: ViewportTransform;
  readonly width: number;
  readonly height: number;
  readonly colors: ThemeDefinition["colors"];
  readonly selected: readonly string[];
  readonly selectedEdge: string | null;
  readonly readonly: boolean;
  readonly scope: string;
  readonly states: ReadonlyMap<string, NodeRunState>;
  readonly runningDocument: boolean;
  readonly preview: NodeDragPreview | null;
  readonly box: FlowRect | null;
  readonly wire: WirePreview | null;
  readonly hover: CanvasHover | null;
}

/** 绘制整帧；调用前上下文已映射到 CSS 像素。 */
export function drawScene(
  context: CanvasRenderingContext2D,
  resources: DrawingResources,
  frame: SceneFrame,
): number {
  const { scene, nodes, camera, width, height, colors, preview } = frame;
  context.clearRect(0, 0, width, height);
  drawBackground(context, camera, width, height, colors);
  let drawn = 0;
  const visit = (scopeId: string, inherited: FlowPoint) => {
    const scope = scene.scopes[scopeId];
    const view = compose(camera, {
      ...scope.transform,
      x: scope.transform.x + inherited.x,
      y: scope.transform.y + inherited.y,
    });
    if (!isRectVisible(scope.bounds, view, width, height) || view.zoom < 0.025)
      return;
    const delta = (id: string): FlowPoint =>
      preview?.scope === scopeId && preview.ids.includes(id)
        ? preview.delta
        : { x: 0, y: 0 };
    context.save();
    context.translate(view.x, view.y);
    context.scale(view.zoom, view.zoom);
    if (scope.parent && view.zoom >= 0.25 && view.zoom <= 16) {
      context.fillStyle = colors.structure;
      context.font = "500 11px system-ui, sans-serif";
      context.fillText(scope.label, scope.bounds.x, scope.bounds.y - 8);
    }
    if (
      !scope.nodes.length &&
      !scene.details[scopeId].endpoints.length &&
      scope.parent &&
      view.zoom >= 0.25 &&
      view.zoom <= 16
    ) {
      context.fillStyle = colors.muted;
      context.font = "13px system-ui, sans-serif";
      context.textAlign = "center";
      context.fillText(
        "双击添加节点",
        scope.bounds.x + scope.bounds.width / 2,
        scope.bounds.y + scope.bounds.height / 2,
      );
      context.textAlign = "left";
    }
    const wire = frame.wire?.scope === scopeId ? frame.wire : null;
    const hovered = frame.hover?.scope === scopeId ? frame.hover : null;
    const edges =
      preview?.scope === scopeId
        ? previewRoutes(scene, preview)
        : scene.details[scopeId].edges;
    const ports = (id: string, rect: FlowRect, color: string) => {
      if (
        frame.readonly ||
        !(
          (hovered?.kind === "node" && hovered.id === id) ||
          wire?.candidate === id ||
          wire?.source.owner === id
        )
      )
        return;
      const tone =
        wire?.candidate === id
          ? wire.error
            ? colors.danger
            : colors.accent
          : color;
      for (const side of ANCHOR_SIDES) {
        const anchor = rectAnchor(rect, side);
        drawPort(
          context,
          anchor.point.x,
          anchor.point.y,
          tone,
          colors.surface,
          view.zoom,
        );
      }
    };
    for (const edge of edges) {
      if (
        wire?.replacing === edge.id ||
        !isRectVisible(edge.bounds, view, width, height, 20)
      )
        continue;
      const selected =
        frame.scope === scopeId && frame.selectedEdge === edge.id;
      const tone =
        edge.status === "blocked"
          ? colors.danger
          : selected
            ? colors.accent
            : colors[nodes.get(edge.source)?.tone ?? "accent"];
      drawEdge(
        context,
        edge,
        tone,
        view.zoom,
        hovered?.kind === "edge" && hovered.id === edge.id,
        selected,
      );
    }
    for (const rect of scope.nodes) {
      const offset = delta(rect.id);
      const placed = { ...rect, x: rect.x + offset.x, y: rect.y + offset.y };
      if (!isRectVisible(placed, view, width, height, 30)) continue;
      const node = nodes.get(rect.id);
      if (!node) continue;
      drawn++;
      drawCard(
        context,
        resources,
        placed,
        node,
        colors,
        view.zoom,
        frame.selected.includes(rect.id),
        Boolean(rect.children.length),
        frame.states.get(rect.id) ??
          (frame.runningDocument ? "waiting" : undefined),
      );
      // 子图递归使用根坐标位移，移动容器不会打散内部布局。
      context.restore();
      for (const child of rect.children)
        visit(child, {
          x: inherited.x + offset.x * scope.transform.zoom,
          y: inherited.y + offset.y * scope.transform.zoom,
        });
      context.save();
      context.translate(view.x, view.y);
      context.scale(view.zoom, view.zoom);
      ports(rect.id, placed, colors[node.tone]);
    }
    for (const endpoint of scene.details[scopeId].endpoints) {
      const offset = delta(endpoint.id);
      const placed = {
        ...endpoint,
        x: endpoint.x + offset.x,
        y: endpoint.y + offset.y,
      };
      if (!isRectVisible(placed, view, width, height, 20)) continue;
      const entry = endpoint.kind === "start";
      drawEndpoint(
        context,
        resources,
        placed,
        entry,
        colors,
        view.zoom,
        frame.selected.includes(endpoint.id),
      );
      ports(endpoint.id, placed, entry ? colors.nodeStart : colors.nodeEnd);
    }
    if (wire) {
      const path = wireRoute(wire, scene.details[scopeId].obstacles);
      drawEdge(
        context,
        path,
        wire.error || path.status === "blocked" ? colors.danger : colors.accent,
        view.zoom,
        true,
      );
      drawEdgeHandle(
        context,
        wire.source,
        colors.accent,
        colors.surface,
        view.zoom,
      );
      drawEdgeHandle(
        context,
        wire.target,
        wire.error ? colors.danger : colors.accent,
        colors.surface,
        view.zoom,
      );
      const message =
        wire.error ??
        (path.status === "blocked" ? "路线受阻，请移动节点或更换连接边" : null);
      if (message) {
        context.font = 12 / view.zoom + "px system-ui, sans-serif";
        context.fillStyle = colors.danger;
        context.fillText(
          message,
          wire.target.point.x + 12 / view.zoom,
          wire.target.point.y - 14 / view.zoom,
        );
      }
    } else if (
      !frame.readonly &&
      frame.scope === scopeId &&
      frame.selectedEdge
    ) {
      const edge = edges.find((edge) => edge.id === frame.selectedEdge);
      if (edge)
        for (const anchor of [edge.sourceAnchor, edge.targetAnchor])
          drawEdgeHandle(
            context,
            anchor,
            colors.accent,
            colors.surface,
            view.zoom,
          );
    }
    context.restore();
  };
  visit(scene.root, { x: 0, y: 0 });
  if (preview && !frame.readonly)
    drawSnapGuides(
      context,
      preview.guides,
      compose(camera, scene.scopes[preview.scope].transform),
      colors.snapGuide,
    );
  context.strokeStyle = colors.accent;
  context.lineWidth = 1;
  if (frame.box) {
    context.globalAlpha = 0.1;
    context.fillStyle = colors.accent;
    context.fillRect(
      frame.box.x,
      frame.box.y,
      frame.box.width,
      frame.box.height,
    );
    context.globalAlpha = 1;
    context.strokeRect(
      frame.box.x,
      frame.box.y,
      frame.box.width,
      frame.box.height,
    );
  }
  return drawn;
}
