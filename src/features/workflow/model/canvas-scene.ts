import {
  routeEdge,
  rectAnchor,
  RoutingObstacles,
  type FlowRect,
  type FlowAnchor,
  type RoutedPath,
} from "../../../flow";
import type { WorkflowFile } from "./contracts";
import { buildScene, type Scene } from "./layout";
import { scopeById } from "./graph";
import { scopeEndpoints, endpointNodeId, type EndpointKind } from "./endpoints";

/** 起止卡片只参与图边界，不写入可执行节点集合。 */
export interface SceneEndpoint extends FlowRect {
  readonly id: string;
  readonly kind: EndpointKind;
}
/** 绘制、命中和端点拖拽共享同一份带圆弧的实际路径。 */
export interface SceneEdge extends RoutedPath {
  readonly id: string;
  readonly source: string;
  readonly target: string;
  readonly sourceAnchor: FlowAnchor;
  readonly targetAnchor: FlowAnchor;
}
export interface ScopeDetails {
  readonly endpoints: readonly SceneEndpoint[];
  readonly edges: readonly SceneEdge[];
  readonly rects: ReadonlyMap<string, FlowRect>;
  readonly obstacles: RoutingObstacles;
}
/** 按不可变文件缓存，不因视口、选择、hover 改变而重新路由。 */
export interface CanvasScene extends Scene {
  readonly root: string;
  readonly details: Readonly<Record<string, ScopeDetails>>;
  readonly maxZoom: number;
}
const scenes = new WeakMap<WorkflowFile, CanvasScene>();
export function buildCanvasScene(file: WorkflowFile): CanvasScene {
  const cached = scenes.get(file);
  if (cached) return cached;
  const scene = buildScene(file);
  const details: Record<string, ScopeDetails> = {};
  for (const geometry of Object.values(scene.scopes)) {
    const scope = scopeById(file, geometry.id),
      endpoints = scopeEndpoints(file, scope.id);
    const elements = [...geometry.nodes, ...endpoints];
    const rects = new Map(elements.map((rect) => [rect.id, rect]));
    const obstacles = new RoutingObstacles(elements);
    const edges = scope.edges.flatMap((edge): SceneEdge[] => {
      const source = endpointNodeId(scope.id, edge.source),
        target = endpointNodeId(scope.id, edge.target);
      const sourceRect = rects.get(source),
        targetRect = rects.get(target);
      if (!sourceRect || !targetRect) return [];
      const layout = file.editor.edges[edge.id];
      const sourceAnchor = rectAnchor(sourceRect, layout.source, source),
        targetAnchor = rectAnchor(targetRect, layout.target, target);
      return [
        {
          id: edge.id,
          source,
          target,
          sourceAnchor,
          targetAnchor,
          ...routeEdge(sourceAnchor, targetAnchor, obstacles),
        },
      ];
    });
    details[scope.id] = { endpoints, edges, rects, obstacles };
  }
  const result: CanvasScene = {
    ...scene,
    root: file.definition.root,
    details,
    maxZoom:
      4 /
      Math.min(
        ...Object.values(scene.scopes).map((scope) => scope.transform.zoom),
      ),
  };
  scenes.set(file, result);
  return result;
}
/** 框选、对齐和定位使用同一份可编辑对象集合。 */
export function scopeElements(scene: CanvasScene, scope: string) {
  return [...scene.scopes[scope].nodes, ...scene.details[scope].endpoints];
}
