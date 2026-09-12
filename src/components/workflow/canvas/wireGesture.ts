import {
  compose,
  screenToWorld,
  rectAnchor,
  facingSide,
  type FlowAnchor,
  type FlowPoint,
  type ViewportTransform,
} from "../../../flow";
import {
  connectionError,
  edgeEndpoint,
  type CanvasScene,
  type WorkflowFile,
} from "../../../features/workflow";
import type { CanvasHit } from "./hitTest";
import type { WirePreview } from "./scene";

/** 手势固定文件快照和未拖动端；松手前不改变图。 */
export interface WireGesture {
  readonly kind: "wire";
  readonly scope: string;
  readonly file: WorkflowFile;
  readonly edge: string | null;
  readonly moving: "source" | "target";
  readonly source: FlowAnchor;
  readonly target: FlowAnchor;
}
/** 普通端口开始新建；选中线的操作点开始改接。 */
export function beginWire(
  hit: CanvasHit,
  scene: CanvasScene,
  file: WorkflowFile,
): WireGesture | null {
  if (hit.kind === "port" && hit.port === "out") {
    const source = rectAnchor(
      scene.details[hit.scope].rects.get(hit.node)!,
      hit.side,
      hit.node,
    );
    return {
      kind: "wire",
      scope: hit.scope,
      file,
      edge: null,
      moving: "target",
      source,
      target: {
        point: source.point,
        side: facingSide(source.point, {
          x: source.point.x - 1,
          y: source.point.y,
        }),
      },
    };
  }
  if (hit.kind === "edge-end") {
    const edge = scene.details[hit.scope].edges.find(
      (edge) => edge.id === hit.edge,
    );
    if (edge)
      return {
        kind: "wire",
        scope: hit.scope,
        file,
        edge: hit.edge,
        moving: hit.end,
        source: edge.sourceAnchor,
        target: edge.targetAnchor,
      };
  }
  return null;
}
// 同一手势固定图快照，同一候选节点不在每次指针移动时重遍历图。
const validations = new WeakMap<WireGesture, Map<string, string | null>>();
/** 拖到卡片主体自动择边；精确端口优先。所有坐标保持在原作用域中。 */
export function previewWire(
  active: WireGesture,
  scene: CanvasScene,
  camera: ViewportTransform,
  screen: FlowPoint,
  hit: CanvasHit,
): WirePreview {
  const fixed = active.moving === "target" ? active.source : active.target;
  const point = screenToWorld(
    screen,
    compose(camera, scene.scopes[active.scope].transform),
  );
  const candidate =
    hit.scope === active.scope &&
    (hit.kind === "node" || hit.kind === "port" || hit.kind === "menu")
      ? hit.node
      : null;
  const rect = candidate
    ? scene.details[active.scope].rects.get(candidate)
    : null;
  const side = rect
    ? hit.kind === "port"
      ? hit.side
      : facingSide(
          { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 },
          fixed.point,
        )
    : facingSide(point, fixed.point);
  const moving: FlowAnchor =
    rect && candidate ? rectAnchor(rect, side, candidate) : { point, side };
  const source = active.moving === "source" ? moving : fixed,
    target = active.moving === "target" ? moving : fixed;
  let error = hit.scope !== active.scope ? "不能跨作用域连线" : null;
  if (candidate) {
    let cached = validations.get(active);
    if (!cached) {
      cached = new Map();
      validations.set(active, cached);
    }
    if (!cached.has(candidate))
      cached.set(
        candidate,
        connectionError(
          active.file,
          active.scope,
          edgeEndpoint(active.scope, source.owner!),
          edgeEndpoint(active.scope, target.owner!),
          active.edge ?? undefined,
        ),
      );
    error = cached.get(candidate)!;
  }
  return {
    scope: active.scope,
    source,
    target,
    replacing: active.edge,
    candidate,
    error,
  };
}
