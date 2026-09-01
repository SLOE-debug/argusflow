import Repeat2 from 'lucide-react/dist/esm/icons/repeat-2.mjs';
import {
  useMemo,
  type ComponentType,
} from 'react';

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
  type WorkflowEdgeData,
  type WorkflowNodeData,
} from '../../../features/workflow';

type WorkflowLoopContainerProps = FlowNodeRendererProps<WorkflowNodeData> & Readonly<{
  /** 普通子节点使用主画布的同一渲染器，保证字段和运行态实时一致。 */
  nodeRenderer: ComponentType<FlowNodeRendererProps<WorkflowNodeData>>;
}>;

type WorkflowLoopBodyDocument = FlowDocument<WorkflowNodeData, WorkflowEdgeData>;

/** While 在父画布中渲染为自动包裹真实子图的只读结构容器。 */
export function WorkflowLoopContainer({
  node,
  nodeRenderer,
  selected,
}: WorkflowLoopContainerProps) {
  /** 渲染器由 loop 注册项调用；判别检查避免错误注册读取不存在字段。 */
  const bodyScopeId = node.data.kind === 'loop' ? node.data.bodyScopeId : '';
  const bodyDocument = useFlowStore((state) => (
    state.documents[bodyScopeId] as WorkflowLoopBodyDocument | undefined
  ));
  if (node.data.kind !== 'loop') return null;

  const surfaceTone = selected
    ? 'border-violet-500 bg-violet-50/80 shadow-[0_10px_30px_rgba(124,58,237,.16)]'
    : 'border-violet-300 bg-violet-50/60 shadow-[0_8px_24px_rgba(124,58,237,.10)]';
  return (
    <div
      className={`relative h-full w-full overflow-visible rounded-xl border-2 ${surfaceTone}`}
      data-loop-scope-id={node.data.bodyScopeId}
    >
      <div
        className="absolute top-1 left-2 z-20 flex max-w-[calc(50%_-_18px)] items-center gap-1 overflow-hidden rounded-md border border-violet-200/80 bg-white/90 px-1.5 py-0.5 text-violet-700 shadow-sm"
        data-loop-label={node.id}
      >
        <Repeat2
          className="size-3 shrink-0"
          aria-hidden="true"
        />
        <strong className="min-w-0 flex-1 truncate text-[10px] leading-3 font-semibold text-violet-900">
          {node.data.label}
        </strong>
        <span className="shrink-0 text-[9px] leading-3 text-violet-500">
          {node.data.maxIterations} 轮
        </span>
      </div>
      <LoopBodyGraph
        document={bodyDocument}
        nodeRenderer={nodeRenderer}
        scopeId={node.data.bodyScopeId}
      />
    </div>
  );
}

type LoopBodyGraphProps = Readonly<{
  document: WorkflowLoopBodyDocument | undefined;
  nodeRenderer: ComponentType<FlowNodeRendererProps<WorkflowNodeData>>;
  scopeId: string;
}>;

/** 按子作用域真实位置、真实节点组件和实际连线渲染一层实时只读子图。 */
function LoopBodyGraph({
  document,
  nodeRenderer: NodeRenderer,
  scopeId,
}: LoopBodyGraphProps) {
  const nodes = document?.nodes ?? [];
  const edges = document?.edges ?? [];
  const layout = resolveWorkflowLoopLayout(nodes);
  const routes = useMemo(() => edges.flatMap((edge) => {
    const result = routeEdge(edge, nodes);
    return result ? [result.route] : [];
  }), [edges, nodes]);
  const bounds = layout.bounds;
  if (!bounds) return null;

  /** 每个作用域拥有独立 SVG marker，避免嵌套预览复用错误的文档内 ID。 */
  const markerId = `workflow-loop-arrow-${scopeId.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
  const bodyTransform = `scale(${WORKFLOW_LOOP_PREVIEW_SCALE})`;
  return (
    <div
      className="pointer-events-none absolute origin-top-left"
      data-loop-body-graph={scopeId}
      style={{
        height: bounds.height,
        left: WORKFLOW_LOOP_BODY_PADDING,
        top: WORKFLOW_LOOP_BODY_TOP_INSET,
        transform: bodyTransform,
        width: bounds.width,
      }}
    >
      <svg
        className="absolute inset-0 overflow-visible"
        height={bounds.height}
        width={bounds.width}
      >
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
            <path
              d="M 0 0 L 10 5 L 0 10 z"
              fill="#8b5cf6"
            />
          </marker>
        </defs>
        <g transform={`translate(${-bounds.x} ${-bounds.y})`}>
          {routes.map((route) => (
            <path
              key={route.edgeId}
              data-loop-edge-id={route.edgeId}
              d={route.path}
              fill="none"
              markerEnd={`url(#${markerId})`}
              stroke="#8b5cf6"
              strokeOpacity="0.65"
              strokeWidth="1.7"
              vectorEffect="non-scaling-stroke"
            />
          ))}
        </g>
      </svg>
      {nodes.map((child) => (
        <div
          key={child.id}
          className="absolute"
          data-loop-child-node-id={child.id}
          style={{
            height: child.size.height,
            transform: `translate(${child.position.x - bounds.x}px, ${child.position.y - bounds.y}px)`,
            width: child.size.width,
          }}
        >
          {child.data.kind === 'loop' ? (
            <WorkflowLoopContainer
              node={child}
              nodeRenderer={NodeRenderer}
              selected={false}
            />
          ) : (
            <NodeRenderer
              node={child}
              selected={false}
            />
          )}
        </div>
      ))}
    </div>
  );
}
