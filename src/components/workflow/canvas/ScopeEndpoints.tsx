import { Play, Square } from "lucide-react";
import { routeEdge, routePath } from "../../../flow/geometry/routing";
import {
  SCOPE_ENDPOINT_LAYOUT,
  type ScopeGeometry,
  type WorkflowFile,
} from "../../../features/workflow";

/** 起止标记属于连线系统，空作用域由画布添加引导承接。 */
export function ScopeEndpoints({
  scope,
  geometry,
  marker,
}: {
  readonly scope: WorkflowFile["definition"]["scopes"][number];
  readonly geometry: ScopeGeometry;
  readonly marker: string;
}) {
  if (!geometry.nodes.length) return null;
  const byId = new Map(geometry.nodes.map((node) => [node.id, node]));
  const entry = scope.entry ? byId.get(scope.entry) : undefined;
  /** 显式跳转、返回及失败节点不使用作用域的自然结束连线。 */
  const terminals = scope.nodes.flatMap((node) => {
    const rect = byId.get(node.id);
    return rect &&
      node.next === null &&
      !["return", "fail", "break", "continue"].includes(node.action.kind)
      ? [rect]
      : [];
  });
  /** 多个自然出口汇入同一结束标记；垂直位置取出口中心范围的中点。 */
  const exitNodes = terminals.length ? terminals : geometry.nodes;
  const exitCenters = exitNodes.map((node) => node.y + node.height / 2);
  const first = entry ?? geometry.nodes[0];
  const { width, height } = SCOPE_ENDPOINT_LAYOUT;
  const start = {
    x: geometry.bounds.x,
    y: first.y + first.height / 2,
  };
  const end = {
    x: geometry.bounds.x + geometry.bounds.width - width,
    y: (Math.min(...exitCenters) + Math.max(...exitCenters)) / 2,
  };
  return (
    <>
      <svg
        className="pointer-events-none absolute left-0 top-0 overflow-visible"
        width={1}
        height={1}
      >
        {entry && (
          <path
            d={routePath(
              routeEdge(
                { x: start.x + width, y: start.y },
                { x: entry.x, y: entry.y + entry.height / 2 },
                geometry.nodes.filter((node) => node.id !== entry.id),
              ),
            )}
            stroke="var(--af-edge)"
            fill="none"
            strokeWidth={1.5}
            markerEnd={"url(#" + marker + ")"}
          />
        )}
        {terminals.map((node) => (
          <path
            key={node.id}
            d={routePath(
              routeEdge(
                { x: node.x + node.width, y: node.y + node.height / 2 },
                { x: end.x, y: end.y },
                geometry.nodes.filter((other) => other.id !== node.id),
              ),
            )}
            stroke="var(--af-edge)"
            fill="none"
            strokeWidth={1.5}
            markerEnd={"url(#" + marker + ")"}
          />
        ))}
      </svg>
      <div
        className="absolute flex items-center justify-center gap-2 rounded-md border border-success/30 bg-surface text-ink shadow-xs"
        style={{ left: start.x, top: start.y - height / 2, width, height }}
      >
        <Play
          size={12}
          fill="currentColor"
          className="text-success"
          aria-hidden="true"
        />
        <span className="text-[11px] font-medium">开始</span>
        <div
          data-port="out"
          data-port-node="$entry"
          title="开始"
          className="absolute -right-1.5 top-1/2 size-3 -translate-y-1/2 cursor-crosshair rounded-full border border-success/50 bg-surface hover:border-success hover:bg-success-soft"
        />
      </div>
      <div
        className="absolute flex items-center justify-center gap-2 rounded-md border border-strong/70 bg-surface text-ink shadow-xs"
        style={{ left: end.x, top: end.y - height / 2, width, height }}
      >
        <Square
          size={10}
          fill="currentColor"
          className="text-muted"
          aria-hidden="true"
        />
        <span className="text-[11px] font-medium">结束</span>
        <div
          data-port="in"
          data-port-node="$exit"
          title="结束"
          className="absolute -left-1.5 top-1/2 size-3 -translate-y-1/2 cursor-crosshair rounded-full border border-strong bg-surface hover:border-accent"
        />
      </div>
    </>
  );
}
