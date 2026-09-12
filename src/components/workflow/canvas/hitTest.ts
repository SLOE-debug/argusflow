import {
  compose,
  contains,
  distanceToRoute,
  rectAnchor,
  ANCHOR_SIDES,
  type FlowAnchorSide,
  screenToWorld,
  type FlowPoint,
  type ViewportTransform,
} from "../../../flow";
import type { CanvasScene } from "../../../features/workflow";
import type { CanvasLocation, NodePresentation } from "./scene";

/** 命中携带所属作用域，禁止依赖 DOM 祖先推断编辑归属。 */
export type CanvasHit = CanvasLocation &
  (
    | { readonly kind: "background" }
    | { readonly kind: "node"; readonly node: string }
    | { readonly kind: "menu"; readonly node: string }
    | {
        readonly kind: "port";
        readonly node: string;
        readonly port: "in" | "out";
        readonly side: FlowAnchorSide;
      }
    | { readonly kind: "edge"; readonly edge: string }
    | {
        readonly kind: "edge-end";
        readonly edge: string;
        readonly end: "source" | "target";
      }
    | { readonly kind: "empty" }
  );

/** 逆绘制顺序检查，子图仅在其可读尺度内响应直接编辑。 */
export function hitTest(
  scene: CanvasScene,
  nodes: ReadonlyMap<string, NodePresentation>,
  camera: ViewportTransform,
  screen: FlowPoint,
  selected?: { readonly scope: string; readonly edge: string | null },
): CanvasHit {
  if (selected?.edge && scene.scopes[selected.scope]) {
    const view = compose(camera, scene.scopes[selected.scope].transform);
    const edge = scene.details[selected.scope].edges.find(
      (edge) => edge.id === selected.edge,
    );
    const point = screenToWorld(screen, view);
    if (edge && view.zoom >= 0.25)
      for (const end of ["source", "target"] as const) {
        const anchor = end === "source" ? edge.sourceAnchor : edge.targetAnchor;
        if (
          Math.hypot(point.x - anchor.point.x, point.y - anchor.point.y) *
            view.zoom <=
          10
        )
          return {
            scope: selected.scope,
            point,
            kind: "edge-end",
            edge: edge.id,
            end,
          };
      }
  }
  const visit = (scopeId: string): CanvasHit | null => {
    const scope = scene.scopes[scopeId];
    const view = compose(camera, scope.transform);
    if (scope.parent && view.zoom < 0.25) return null;
    const point = screenToWorld(screen, view);
    const location = { scope: scopeId, point };
    const detail = scene.details[scopeId];
    const portHit = (
      id: string,
      rect: import("../../../flow").FlowRect,
      port: "in" | "out",
    ): CanvasHit | null => {
      if (view.zoom < 0.25) return null;
      for (const side of ANCHOR_SIDES) {
        const anchor = rectAnchor(rect, side);
        if (
          Math.hypot(point.x - anchor.point.x, point.y - anchor.point.y) *
            view.zoom <=
          10
        )
          return { ...location, kind: "port", node: id, port, side };
      }
      return null;
    };
    for (const endpoint of [...detail.endpoints].reverse()) {
      const port = endpoint.kind === "start" ? "out" : "in";
      const hit = portHit(endpoint.id, endpoint, port);
      if (hit) return hit;
      if (contains(endpoint, point))
        return { ...location, kind: "node", node: endpoint.id };
    }
    for (let i = scope.nodes.length - 1; i >= 0; i--) {
      const node = scope.nodes[i];
      const port = portHit(
        node.id,
        node,
        nodes.get(node.id)?.terminal ? "in" : "out",
      );
      if (port) return port;
      if (!contains(node, point)) continue;
      for (let child = node.children.length - 1; child >= 0; child--) {
        const result = visit(node.children[child]);
        if (result) return result;
      }
      if (point.x > node.x + node.width - 32 && point.y < node.y + 32)
        return { ...location, kind: "menu", node: node.id };
      return { ...location, kind: "node", node: node.id };
    }
    for (let i = detail.edges.length - 1; i >= 0; i--) {
      const edge = detail.edges[i];
      if (distanceToRoute(point, edge) * view.zoom <= 8)
        return {
          ...location,
          kind: "edge",
          edge: edge.id,
          connection: { kind: "insert", edge: edge.id },
        };
    }
    if (contains(scope.bounds, point) || !scope.parent)
      return {
        ...location,
        kind:
          scope.nodes.length || detail.endpoints.length
            ? "background"
            : "empty",
      };
    return null;
  };
  return visit(scene.root)!;
}
