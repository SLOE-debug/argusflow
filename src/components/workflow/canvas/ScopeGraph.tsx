import { memo } from "react";
import { compose, isRectVisible, type ViewportTransform } from "../../../flow";
import {
  nodeKind,
  nodeTitle,
  nodeSummary,
  scopeById,
  type RunSnapshot,
  type Scene,
  type WorkflowFile,
} from "../../../features/workflow";
import { NodeIcon } from "../presentation/NodeIcon";
import { nodeTone } from "../presentation/nodeTone";
import type { NodeRunState } from "../execution/nodeStates";
import { routeEdge, routePath } from "../../../flow/geometry/routing";
import { ScopeEndpoints } from "./ScopeEndpoints";

interface GraphProps {
  readonly file: WorkflowFile;
  readonly scene: Scene;
  readonly scopeId: string;
  readonly activeScope: string;
  readonly selected: readonly string[];
  readonly rootView: ViewportTransform;
  readonly width: number;
  readonly height: number;
  readonly run: RunSnapshot | null;
  readonly states: ReadonlyMap<string, NodeRunState>;
}
export const ScopeGraph = memo(function ScopeGraph(props: GraphProps) {
  const {
    file,
    scene,
    scopeId,
    activeScope,
    selected,
    rootView,
    width,
    height,
    run,
    states,
  } = props;
  const geometry = scene.scopes[scopeId];
  const scope = scopeById(file, scopeId);
  const view = compose(rootView, geometry.transform);
  const active = scopeId === activeScope;
  const marker = "arrow-" + scopeId;
  const byId = new Map(geometry.nodes.map((node) => [node.id, node]));
  const edges = scope.nodes.flatMap((node) => {
    const source = byId.get(node.id),
      target = node.next ? byId.get(node.next) : undefined;
    if (!source || !target) return [];
    const bounds = {
      x: Math.min(source.x, target.x),
      y: Math.min(source.y, target.y),
      width:
        Math.abs(target.x - source.x) + Math.max(source.width, target.width),
      height:
        Math.abs(target.y - source.y) + Math.max(source.height, target.height),
    };
    if (!isRectVisible(bounds, view, width, height)) return [];
    return [
      {
        source: node.id,
        path: routePath(
          routeEdge(
            { x: source.x + source.width, y: source.y + source.height / 2 },
            { x: target.x, y: target.y + target.height / 2 },
            geometry.nodes.filter(
              (rect) => rect.id !== source.id && rect.id !== target.id,
            ),
          ),
        ),
      },
    ];
  });
  return (
    <div
      className="absolute left-0 top-0 origin-top-left"
      style={{
        transform:
          "translate(" +
          geometry.local.x +
          "px," +
          geometry.local.y +
          "px) scale(" +
          geometry.local.zoom +
          ")",
      }}
      data-scope-graph={scopeId}
    >
      <svg
        className="pointer-events-none absolute left-0 top-0 overflow-visible"
        width={1}
        height={1}
      >
        <defs>
          <marker
            id={marker}
            markerWidth="6"
            markerHeight="6"
            refX="5"
            refY="3"
            orient="auto"
          >
            <path d="M 0 0 L 6 3 L 0 6 Z" fill="var(--af-edge)" />
          </marker>
        </defs>
        {edges.map((edge) => (
          <g key={edge.source}>
            <path
              d={edge.path}
              fill="none"
              stroke="var(--af-edge)"
              strokeWidth={1.5}
              markerEnd={"url(#" + marker + ")"}
            />
            {active && (
              <path
                data-edge-source={edge.source}
                d={edge.path}
                fill="none"
                stroke="transparent"
                strokeWidth={16}
                className="pointer-events-auto cursor-copy"
              />
            )}
          </g>
        ))}
      </svg>
      {geometry.nodes
        .filter((node) => isRectVisible(node, view, width, height))
        .map((rect) => {
          const node = scope.nodes.find((item) => item.id === rect.id)!;
          const container = rect.children.length > 0;
          const chosen = active && selected.includes(node.id);
          const state =
            states.get(node.id) ??
            (run?.documents.includes(file.id) ? "waiting" : null);
          const tone = chosen
            ? "border-accent ring-2 ring-accent/20"
            : state === "failed"
              ? "border-danger"
              : container
                ? "border-structure/50"
                : "border-strong/70";
          return (
            <div
              key={node.id}
              data-node={node.id}
              data-node-scope={scopeId}
              className={
                "absolute origin-top-left rounded-lg border shadow-sm " +
                tone +
                " " +
                (container ? "bg-structure-soft/50" : "bg-surface")
              }
              style={{
                left: rect.x,
                top: rect.y,
                width: rect.width,
                height: rect.height,
              }}
            >
              <div
                className={
                  "flex items-center gap-2.5 px-3 " +
                  (container ? "h-10" : "h-full")
                }
              >
                <span className={nodeTone(nodeKind(node)) + " shrink-0"}>
                  <NodeIcon kind={nodeKind(node)} size={18} />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[12px] font-semibold">
                    {nodeTitle(file, node)}
                  </div>
                  {!container && view.zoom > 0.4 && (
                    <div className="mt-0.5 truncate text-[10px] text-muted">
                      {nodeSummary(node)}
                    </div>
                  )}
                </div>
                {state && (
                  <span
                    title={
                      state === "running"
                        ? "执行中"
                        : state === "completed"
                          ? "已完成"
                          : state === "waiting"
                            ? "等待执行"
                            : "失败"
                    }
                    className={
                      "size-1.5 shrink-0 rounded-full " +
                      (state === "failed"
                        ? "bg-danger"
                        : state === "running"
                          ? "animate-pulse bg-accent"
                          : state === "waiting"
                            ? "bg-strong"
                            : "bg-success")
                    }
                  />
                )}
              </div>
              {active && (
                <>
                  <div
                    data-port="in"
                    data-port-node={node.id}
                    className="absolute -left-1.5 top-1/2 z-10 size-3 -translate-y-1/2 cursor-crosshair rounded-full border border-strong bg-surface hover:border-accent"
                  />
                  <div
                    data-port="out"
                    data-port-node={node.id}
                    className={
                      "absolute -right-1.5 top-1/2 z-10 size-3 -translate-y-1/2 cursor-crosshair rounded-full border border-strong bg-surface hover:border-accent " +
                      (["return", "fail", "break", "continue"].includes(
                        node.action.kind,
                      )
                        ? "hidden"
                        : "")
                    }
                  />
                </>
              )}
              {container &&
                rect.children.map((childId) => {
                  const child = scene.scopes[childId];
                  const showDetails = view.zoom * child.local.zoom >= 0.14;
                  return (
                    <div key={childId} className="contents">
                      <span
                        className="pointer-events-none absolute text-[10px] font-medium text-structure"
                        style={{
                          left: 24,
                          top:
                            child.local.y -
                            rect.y +
                            child.bounds.y * child.local.zoom -
                            17,
                        }}
                      >
                        {child.label}
                      </span>
                      {showDetails && (
                        <div
                          className="absolute left-0 top-0"
                          style={{
                            transform:
                              "translate(" + -rect.x + "px," + -rect.y + "px)",
                          }}
                        >
                          <ScopeGraph {...props} scopeId={childId} />
                        </div>
                      )}
                    </div>
                  );
                })}
            </div>
          );
        })}
      {active && (
        <ScopeEndpoints scope={scope} geometry={geometry} marker={marker} />
      )}
    </div>
  );
});
