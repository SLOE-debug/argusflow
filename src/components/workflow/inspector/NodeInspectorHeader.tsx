import type { NodeRunState } from '../../../features/workflow';
import { Input } from '../../ui';

type NodeInspectorHeaderProps = Readonly<{
  /** 面向用户的节点名称。 */
  label: string;
  /** 随配置即时更新的任务摘要。 */
  summary: string;
  /** 当前节点执行状态。 */
  runState: NodeRunState;
  /** 当前节点是否存在配置错误。 */
  invalid: boolean;
  /** 写回节点名称。 */
  onLabelChange: (label: string) => void;
}>;

/** 所有节点共用的名称、任务摘要和状态头部。 */
export function NodeInspectorHeader({
  label,
  summary,
  runState,
  invalid,
  onLabelChange,
}: NodeInspectorHeaderProps) {
  const status = resolveHeaderStatus(runState, invalid);
  return (
    <section className="border-b border-slate-200 px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Input
          aria-label="节点名称"
          value={label}
          containerClassName="min-w-0 flex-1 border-transparent bg-transparent px-0 focus-within:border-blue-400"
          className="text-[15px] font-semibold text-slate-900"
          onChange={(event) => onLabelChange(event.target.value)}
        />
        <span className={`mt-1 shrink-0 rounded-full px-2 py-1 text-[10px] font-medium ${status.className}`}>
          {status.label}
        </span>
      </div>
      <p className="mt-1 text-[11px] leading-[17px] text-slate-600">{summary}</p>
    </section>
  );
}

/** 状态始终同时使用文字与颜色，避免依赖颜色独立表达含义。 */
function resolveHeaderStatus(
  runState: NodeRunState,
  invalid: boolean,
): Readonly<{ label: string; className: string }> {
  if (invalid) return { label: '需要修改', className: 'bg-rose-50 text-rose-700' };
  switch (runState) {
    case 'running':
      return { label: '正在运行', className: 'bg-blue-50 text-blue-700' };
    case 'pending':
      return { label: '等待运行', className: 'bg-slate-100 text-slate-600' };
    case 'success':
      return { label: '执行成功', className: 'bg-emerald-50 text-emerald-700' };
    case 'error':
      return { label: '执行失败', className: 'bg-rose-50 text-rose-700' };
    case 'skipped':
      return { label: '未执行', className: 'bg-slate-100 text-slate-500' };
    case 'idle':
      return { label: '可编辑', className: 'bg-slate-100 text-slate-600' };
  }
}
