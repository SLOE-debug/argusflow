import BarChart3 from 'lucide-react/dist/esm/icons/chart-column.mjs';
import Circle from 'lucide-react/dist/esm/icons/circle.mjs';
import MoreHorizontal from 'lucide-react/dist/esm/icons/ellipsis.mjs';
import Pause from 'lucide-react/dist/esm/icons/pause.mjs';
import Play from 'lucide-react/dist/esm/icons/play.mjs';
import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import RotateCw from 'lucide-react/dist/esm/icons/rotate-cw.mjs';
import Search from 'lucide-react/dist/esm/icons/search.mjs';
import Settings2 from 'lucide-react/dist/esm/icons/settings-2.mjs';

import {
  WORKFLOW_TASK_ROWS,
  type WorkflowTaskRow,
  type WorkflowTaskStatus,
} from './workflowTaskData';

/** 状态徽标的文字、颜色和进度强调。 */
const STATUS_PRESENTATION = {
  running: {
    label: '运行中',
    badge: 'bg-blue-50 text-blue-600',
    dot: 'bg-blue-500',
    progress: 'bg-blue-500',
  },
  success: {
    label: '成功',
    badge: 'bg-emerald-50 text-emerald-600',
    dot: 'bg-emerald-500',
    progress: 'bg-emerald-500',
  },
  failed: {
    label: '失败',
    badge: 'bg-rose-50 text-rose-600',
    dot: 'bg-rose-500',
    progress: 'bg-rose-500',
  },
  paused: {
    label: '已暂停',
    badge: 'bg-amber-50 text-amber-600',
    dot: 'bg-amber-500',
    progress: 'bg-amber-500',
  },
} as const satisfies Readonly<
  Record<WorkflowTaskStatus, Readonly<{
    label: string;
    badge: string;
    dot: string;
    progress: string;
  }>>
>;

/** 参考图底部任务区域中的操作条和五行任务表格。 */
export function WorkflowTaskTable() {
  return (
    <div className="flex min-h-0 flex-1 flex-col bg-white">
      <TaskToolbar />
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full min-w-[790px] table-fixed border-collapse text-left text-[12px] text-slate-600">
          <thead className="sticky top-0 z-10 h-8 bg-slate-50 text-[11px] font-medium text-slate-700">
            <tr className="border-y border-slate-200">
              <th className="w-9 px-3"><SelectionBox label="选择全部任务" /></th>
              <th className="w-[140px] px-1">任务名称</th>
              <th className="w-[108px] border-l border-slate-200 px-3">状态</th>
              <th className="w-24 border-l border-slate-200 px-3">运行者</th>
              <th className="w-[164px] border-l border-slate-200 px-3">最近运行时间</th>
              <th className="w-24 border-l border-slate-200 px-3">耗时</th>
              <th className="w-36 border-l border-slate-200 px-3">成功次数/总次数</th>
              <th className="w-24 border-l border-slate-200 px-3">操作</th>
            </tr>
          </thead>
          <tbody>
            {WORKFLOW_TASK_ROWS.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** 任务列表上方的紧凑批量操作区。 */
function TaskToolbar() {
  return (
    <div className="flex h-10 shrink-0 items-center gap-2 border-t border-slate-100 px-2">
      <button
        type="button"
        className="flex h-7 items-center gap-1 rounded-[4px] bg-blue-600 px-2.5 text-[11px] font-semibold text-white hover:bg-blue-700"
      >
        <Plus className="size-3.5 shrink-0" aria-hidden="true" />
        新建任务
      </button>
      <TaskActionButton label="启动" icon={Play} disabled />
      <TaskActionButton label="暂停" icon={Pause} disabled />
      <TaskActionButton label="停止" icon={Circle} disabled />
      <button
        type="button"
        className="flex h-7 items-center gap-1 rounded-[4px] border border-slate-300 bg-white px-2.5 text-[11px] text-slate-700"
      >
        更多操作
        <span className="text-[9px]">⌄</span>
      </button>
      <label className="ml-auto flex h-7 w-40 items-center rounded-[4px] border border-slate-300 bg-white px-2 text-slate-400 focus-within:border-blue-400">
        <Search className="size-3.5 shrink-0" aria-hidden="true" />
        <input
          aria-label="搜索任务名称"
          placeholder="搜索任务名称"
          className="min-w-0 flex-1 border-0 bg-transparent pl-1.5 text-[11px] text-slate-700 outline-none placeholder:text-slate-400"
        />
      </label>
      <button
        type="button"
        className="flex h-7 w-28 items-center justify-between rounded-[4px] border border-slate-300 bg-white px-2.5 text-[11px] text-slate-700"
      >
        全部状态
        <span className="text-[9px]">⌄</span>
      </button>
      <button
        type="button"
        aria-label="任务列表设置"
        className="flex size-7 items-center justify-center rounded-[4px] border border-slate-300 text-slate-600"
      >
        <Settings2 className="size-3.5 shrink-0" aria-hidden="true" />
      </button>
    </div>
  );
}

type TaskActionButtonProps = Readonly<{
  label: string;
  icon: typeof Play;
  disabled?: boolean;
}>;

/** 批量操作区的统一次级按钮。 */
function TaskActionButton({ label, icon: Icon, disabled }: TaskActionButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      className="flex h-7 items-center gap-1 rounded-[4px] border border-slate-300 bg-white px-2.5 text-[11px] text-slate-600 disabled:opacity-40"
    >
      <Icon className="size-3 shrink-0" aria-hidden="true" />
      {label}
    </button>
  );
}

/** 单行任务及其状态进度和快捷操作。 */
function TaskRow({ task }: Readonly<{ task: WorkflowTaskRow }>) {
  const status = STATUS_PRESENTATION[task.status];
  /** 已完成比例只用于参考图中的细进度条。 */
  const progress = task.succeeded !== null && task.total !== null && task.total !== Infinity
    ? Math.min(100, task.succeeded / task.total * 100)
    : 0;

  return (
    <tr className="h-[37px] border-b border-slate-200 bg-white hover:bg-blue-50/30">
      <td className="px-3"><SelectionBox label={`选择任务 ${task.name}`} /></td>
      <td className="truncate px-1 font-medium text-slate-700">{task.name}</td>
      <td className="px-3"><StatusBadge taskStatus={task.status} /></td>
      <td className="px-3">{task.operator}</td>
      <td className="px-3 tabular-nums">{task.lastRun}</td>
      <td className="px-3 tabular-nums">{task.duration}</td>
      <td className="px-3">
        <div className="flex items-center gap-2">
          <span className="shrink-0 tabular-nums">
            {task.succeeded ?? '—'} / {task.total === Infinity ? '∞' : task.total ?? '—'}
          </span>
          {progress > 0 ? (
            <span className="h-1 w-12 overflow-hidden rounded-full bg-slate-200">
              <span
                className={`block h-full rounded-full ${status.progress}`}
                style={{ width: `${progress}%` }}
              />
            </span>
          ) : null}
        </div>
      </td>
      <td className="px-3">
        <div className="flex items-center gap-3 text-slate-600">
          {task.status === 'failed' ? (
            <RotateCw className="size-3.5 shrink-0" aria-label="重新运行" />
          ) : (
            <Play className="size-3.5 shrink-0" aria-label="运行任务" />
          )}
          {task.status === 'running' ? (
            <Pause className="size-3.5 shrink-0" aria-label="暂停任务" />
          ) : task.status === 'success' ? (
            <BarChart3 className="size-3.5 shrink-0" aria-label="查看结果" />
          ) : null}
          <MoreHorizontal className="size-4 shrink-0" aria-label="更多操作" />
        </div>
      </td>
    </tr>
  );
}

/** 带状态点的紧凑任务徽标。 */
function StatusBadge({ taskStatus }: Readonly<{ taskStatus: WorkflowTaskStatus }>) {
  const status = STATUS_PRESENTATION[taskStatus];
  return (
    <span className={`inline-flex h-6 items-center gap-1 rounded-full px-2 ${status.badge}`}>
      <span className={`size-1.5 rounded-full ${status.dot}`} />
      {status.label}
    </span>
  );
}

/** 表格选择框保持 14px 的桌面控件密度。 */
function SelectionBox({ label }: Readonly<{ label: string }>) {
  return (
    <input
      type="checkbox"
      aria-label={label}
      className="size-3.5 appearance-none rounded-[3px] border border-slate-300 bg-white checked:border-blue-600 checked:bg-blue-600"
    />
  );
}
