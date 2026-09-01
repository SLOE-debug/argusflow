import CircleCheck from 'lucide-react/dist/esm/icons/circle-check.mjs';
import CircleX from 'lucide-react/dist/esm/icons/circle-x.mjs';
import Clock3 from 'lucide-react/dist/esm/icons/clock-3.mjs';
import LoaderCircle from 'lucide-react/dist/esm/icons/loader-circle.mjs';
import MinusCircle from 'lucide-react/dist/esm/icons/circle-minus.mjs';

import type { FlowNodeRendererProps, NodeDefinition } from '../../../flow';
import type { RunSnapshotNodeData } from '../../../features/workflow';
import { RunSnapshotLoopContainer } from './RunSnapshotLoopContainer';

const STATE_PRESENTATION = {
  idle: { label: '未运行', tone: 'text-slate-400', icon: MinusCircle },
  pending: { label: '等待', tone: 'text-slate-400', icon: Clock3 },
  running: { label: '运行中', tone: 'text-blue-600', icon: LoaderCircle },
  success: { label: '成功', tone: 'text-emerald-600', icon: CircleCheck },
  error: { label: '失败', tone: 'text-rose-600', icon: CircleX },
  skipped: { label: '未经过', tone: 'text-slate-400', icon: MinusCircle },
} as const;

/** 运行快照节点只呈现冻结名称、类型和游标处累计状态。 */
function RunSnapshotNodeCard({ node, selected }: FlowNodeRendererProps<RunSnapshotNodeData>) {
  if (node.data.structure.type === 'loop') {
    return (
      <RunSnapshotLoopContainer
        node={node}
        nodeRenderer={RunSnapshotNodeCard}
        selected={selected}
      />
    );
  }
  return <RunSnapshotAtomicNode node={node} selected={selected} />;
}

/** 普通节点卡片与 While 内部节点共用同一运行状态表达。 */
function RunSnapshotAtomicNode({ node, selected }: FlowNodeRendererProps<RunSnapshotNodeData>) {
  const presentation = STATE_PRESENTATION[node.data.runState];
  const StateIcon = presentation.icon;
  return (
    <article
      className={
        'flex h-full w-full items-center gap-3 rounded-lg border bg-white px-4 shadow-sm ' +
        (selected ? 'border-blue-500 ring-2 ring-blue-100' : 'border-slate-200')
      }
    >
      <StateIcon
        className={`size-5 shrink-0 ${presentation.tone} ${node.data.runState === 'running' ? 'animate-spin' : ''}`}
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1">
        <h3 className="truncate text-[13px] font-semibold text-slate-800">{node.data.label}</h3>
        <p className="mt-0.5 truncate text-[12px] text-slate-500">
          {friendlyType(node.data.typeId)}
        </p>
      </div>
      <div className={`shrink-0 text-right text-[12px] font-medium ${presentation.tone}`}>
        <div>{presentation.label}</div>
        {node.data.executionCount > 1 ? <div>第 {node.data.executionCount} 次</div> : null}
      </div>
    </article>
  );
}

/** 运行快照只需要一个业务无关的节点渲染注册项。 */
export const RUN_SNAPSHOT_NODE_REGISTRY = {
  run_snapshot: {
    kind: 'run_snapshot',
    title: '运行节点',
    defaultSize: { width: 212, height: 64 },
    component: RunSnapshotNodeCard,
    creatable: false,
    canStartConnection: false,
    canEndConnection: false,
    copyable: false,
  },
} satisfies Readonly<Record<'run_snapshot', NodeDefinition<RunSnapshotNodeData>>>;

function friendlyType(typeId: string): string {
  return typeId
    .replace(/^argus\./, '')
    .replaceAll('_', ' ');
}
