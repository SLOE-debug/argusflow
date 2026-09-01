import CircleCheck from 'lucide-react/dist/esm/icons/circle-check.mjs';
import PlayCircle from 'lucide-react/dist/esm/icons/circle-play.mjs';
import Repeat2 from 'lucide-react/dist/esm/icons/repeat-2.mjs';

import {
  useFlowStore,
  type FlowNode,
  type FlowNodeRendererProps,
} from '../../../flow';
import type { WorkflowNodeData } from '../../../features/workflow';

/** While 父层预览最多展示的直接子节点数量，避免复杂正文挤成不可读缩略图。 */
const MAX_PREVIEW_NODES = 7;

/** While 在父画布中渲染为可缩放进入的结构容器和一层只读预览。 */
export function WorkflowLoopContainer({
  node,
  selected,
}: FlowNodeRendererProps<WorkflowNodeData>) {
  /** 渲染器由 loop 注册项调用；保留判别检查保证错误注册不会读取不存在字段。 */
  const bodyScopeId = node.data.kind === 'loop' ? node.data.bodyScopeId : '';
  const bodyNodes = useFlowStore((state) => (
    state.documents[bodyScopeId]?.nodes as ReadonlyArray<FlowNode<WorkflowNodeData>> | undefined
  ));
  if (node.data.kind !== 'loop') return null;

  const surfaceTone = selected
    ? 'border-violet-500 bg-violet-50/80 shadow-[0_10px_30px_rgba(124,58,237,.16)]'
    : 'border-violet-300 bg-violet-50/60 shadow-[0_8px_24px_rgba(124,58,237,.10)]';
  return (
    <div
      className={`relative h-full w-full overflow-hidden rounded-2xl border-2 ${surfaceTone}`}
      data-loop-scope-id={node.data.bodyScopeId}
    >
      <div className="flex h-12 items-center justify-between border-b border-violet-200 bg-white/85 px-4">
        <div className="flex min-w-0 items-center gap-2 text-violet-900">
          <span className="flex size-7 items-center justify-center rounded-lg bg-violet-100">
            <Repeat2 className="size-4" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <strong className="block truncate text-xs font-semibold">{node.data.label}</strong>
            <span className="block text-[10px] text-violet-500">
              最多 {node.data.maxIterations} 轮 · 双击或继续放大进入
            </span>
          </div>
        </div>
        <span className="rounded-full bg-violet-100 px-2 py-1 text-[10px] font-medium text-violet-700">
          While
        </span>
      </div>
      <LoopBodyPreview nodes={bodyNodes ?? []} />
    </div>
  );
}

/** 按子图位置顺序展示直接子节点；嵌套 While 只显示为单个摘要容器。 */
function LoopBodyPreview({
  nodes,
}: Readonly<{ nodes: ReadonlyArray<FlowNode<WorkflowNodeData>> }>) {
  const visibleNodes = [...nodes]
    .sort((left, right) => left.position.x - right.position.x || left.position.y - right.position.y)
    .slice(0, MAX_PREVIEW_NODES);
  return (
    <div className="absolute inset-x-4 top-16 bottom-4 overflow-hidden rounded-xl border border-dashed border-violet-300 bg-white/55 p-3">
      <div className="flex h-full items-center gap-2 overflow-hidden">
        {visibleNodes.map((child, index) => (
          <div key={child.id} className="contents">
            {index > 0 ? <span className="h-px min-w-2 flex-1 bg-violet-200" aria-hidden="true" /> : null}
            <PreviewNode node={child} />
          </div>
        ))}
        {nodes.length > MAX_PREVIEW_NODES ? (
          <span className="shrink-0 text-[10px] font-medium text-violet-500">
            +{nodes.length - MAX_PREVIEW_NODES}
          </span>
        ) : null}
      </div>
    </div>
  );
}

/** 为固定边界、普通步骤与嵌套 While 选择简短且可辨认的一层预览。 */
function PreviewNode({ node }: Readonly<{ node: FlowNode<WorkflowNodeData> }>) {
  const Icon = node.data.kind === 'loopEntry'
    ? PlayCircle
    : node.data.kind === 'loopComplete'
      ? CircleCheck
      : Repeat2;
  const tone = node.data.kind === 'loopComplete'
    ? 'border-emerald-200 text-emerald-700'
    : node.data.kind === 'loopContinue'
      ? 'border-amber-200 text-amber-700'
      : 'border-violet-200 text-violet-700';
  return (
    <span className={`flex min-w-0 max-w-24 shrink items-center gap-1 rounded-md border bg-white px-2 py-1.5 ${tone}`}>
      <Icon className="size-3 shrink-0" aria-hidden="true" />
      <span className="truncate text-[9px] font-medium">{node.data.label}</span>
    </span>
  );
}
