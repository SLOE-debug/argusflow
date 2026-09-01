import Repeat2 from 'lucide-react/dist/esm/icons/repeat-2.mjs';
import { useMemo, type ComponentType } from 'react';

import {
  routeEdge,
  useFlowStore,
  type FlowDocument,
  type FlowNodeRendererProps,
} from '../../../flow';
import {
  resolveWorkflowLoopLayout,
  WORKFLOW_LOOP_BODY_PADDING,
  WORKFLOW_LOOP_BODY_TOP_INSET,
  WORKFLOW_LOOP_PREVIEW_SCALE,
  type RunSnapshotEdgeData,
  type RunSnapshotNodeData,
} from '../../../features/workflow';

type RunSnapshotDocument = FlowDocument<RunSnapshotNodeData, RunSnapshotEdgeData>;

type RunSnapshotLoopContainerProps = FlowNodeRendererProps<RunSnapshotNodeData> & Readonly<{
  /** 子节点继续使用统一运行快照渲染器，从而自然支持嵌套 While。 */
  nodeRenderer: ComponentType<FlowNodeRendererProps<RunSnapshotNodeData>>;
}>;

/** While 在根回放画布内直接展开真实子作用域，不再清空父流程切换文档。 */
export function RunSnapshotLoopContainer({
  node,
  nodeRenderer: NodeRenderer,
  selected,
}: RunSnapshotLoopContainerProps) {
  const structure = node.data.structure;
  const document = useFlowStore((state) => (
    structure.type === 'loop'
      ? state.documents[structure.bodyScopeId] as RunSnapshotDocument | undefined
      : undefined
  ));
  const selectedNodeIds = useFlowStore((state) => state.selectedNodeIds);
  const activeEdgeIds = useFlowStore((state) => state.activeEdgeIds);
  const nodes = document?.nodes ?? [];
  const edges = document?.edges ?? [];
  const layout = resolveWorkflowLoopLayout(nodes);
  const routes = useMemo(() => edges.flatMap((edge) => {
    const result = routeEdge(edge, nodes);
    return result ? [result.route] : [];
  }), [edges, nodes]);
  if (structure.type !== 'loop') return null;

  const bounds = layout.bounds;
  const markerId = `run-loop-arrow-${structure.bodyScopeId.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
  const descendantSelected = nodes.some((child) => selectedNodeIds.has(child.id));
  const surfaceTone = selected || descendantSelected
    ? 'border-violet-500 bg-violet-50/80 shadow-[0_10px_30px_rgba(124,58,237,.16)]'
    : 'border-violet-300 bg-violet-50/60 shadow-[0_8px_24px_rgba(124,58,237,.10)]';

  return (
    <article
      className={`relative h-full w-full overflow-visible rounded-xl border-2 ${surfaceTone}`}
      data-run-loop-scope-id={structure.bodyScopeId}
    >
      <div className="absolute top-1 left-2 z-20 flex max-w-[calc(65%_-_18px)] items-center gap-1 overflow-hidden rounded-md border border-violet-200/80 bg-white/95 px-1.5 py-0.5 text-violet-700 shadow-sm">
        <Repeat2 className="size-3 shrink-0" aria-hidden="true" />
        <strong className="min-w-0 flex-1 truncate text-[11px] leading-3 font-semibold text-violet-900">
          {node.data.label}
        </strong>
        <span className="shrink-0 text-[10px] leading-3 text-violet-600">
          {structure.maxIterations} 轮
        </span>
      </div>
      {bounds ? (
        <div
          className="pointer-events-none absolute origin-top-left"
          data-run-loop-body={structure.bodyScopeId}
          style={{
            height: bounds.height,
            left: WORKFLOW_LOOP_BODY_PADDING,
            top: WORKFLOW_LOOP_BODY_TOP_INSET,
            transform: `scale(${WORKFLOW_LOOP_PREVIEW_SCALE})`,
            width: bounds.width,
          }}
        >
          <svg className="absolute inset-0 overflow-visible" height={bounds.height} width={bounds.width}>
            <defs>
              <marker
                id={markerId}
                markerHeight="7"
                markerUnits="userSpaceOnUse"
                markerWidth="7"
                orient="auto-start-reverse"
                refX="9"
                refY="5"
                viewBox="0 0 10 10"
              >
                <path d="M 0 0 L 10 5 L 0 10 z" fill="#6366f1" />
              </marker>
            </defs>
            <g transform={`translate(${-bounds.x} ${-bounds.y})`}>
              {routes.map((route) => {
                const active = Object.hasOwn(activeEdgeIds, route.edgeId);
                return (
                  <path
                    key={route.edgeId}
                    d={route.path}
                    fill="none"
                    markerEnd={`url(#${markerId})`}
                    stroke={active ? '#2563eb' : '#8b5cf6'}
                    strokeOpacity={active ? 1 : 0.6}
                    strokeWidth={active ? 2.5 : 1.7}
                    vectorEffect="non-scaling-stroke"
                  />
                );
              })}
            </g>
          </svg>
          {nodes.map((child) => (
            <div
              key={child.id}
              className="absolute"
              data-run-loop-child-node-id={child.id}
              style={{
                height: child.size.height,
                transform: `translate(${child.position.x - bounds.x}px, ${child.position.y - bounds.y}px)`,
                width: child.size.width,
              }}
            >
              <NodeRenderer
                node={child}
                selected={selectedNodeIds.has(child.id)}
              />
            </div>
          ))}
        </div>
      ) : null}
    </article>
  );
}
