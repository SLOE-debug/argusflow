import ArrowRight from 'lucide-react/dist/esm/icons/arrow-right.mjs';
import CircleAlert from 'lucide-react/dist/esm/icons/circle-alert.mjs';
import ListChecks from 'lucide-react/dist/esm/icons/list-checks.mjs';
import Workflow from 'lucide-react/dist/esm/icons/workflow.mjs';
import type { LucideIcon } from 'lucide-react';

import type {
  ExecutionEvent,
  ValidationReport,
} from '../../../features/workflow';

type WorkflowOverviewProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** 当前会话已接收的运行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 最近一次结构校验结果。 */
  report: ValidationReport | null;
  /** 返回当前工作流编辑器。 */
  onOpenEditor: () => void;
}>;

/** 中央 Home 入口对应的工作流概览和列表。 */
export function WorkflowOverview({
  workflowName,
  events,
  report,
  onOpenEditor,
}: WorkflowOverviewProps) {
  const validationStatus = report === null
    ? { label: '尚未检查', tone: 'bg-amber-50 text-amber-700' }
    : report.valid
      ? { label: '检查通过', tone: 'bg-emerald-50 text-emerald-700' }
      : { label: `${report.issues.length} 个问题`, tone: 'bg-rose-50 text-rose-700' };

  return (
    <section className="h-full min-h-0 overflow-y-auto bg-slate-50 p-5">
      <div className="mx-auto max-w-5xl">
        <div>
          <h1 className="text-lg font-semibold text-slate-900">工作流</h1>
          <p className="mt-1 text-[12px] text-slate-500">查看流程和最近的运行情况。</p>
        </div>

        <div className="mt-5 grid grid-cols-3 gap-3">
          <OverviewMetric
            icon={Workflow}
            label="工作流"
            value="1"
          />
          <OverviewMetric
            icon={ListChecks}
            label="运行记录"
            value={String(events.length)}
          />
          <OverviewMetric
            icon={CircleAlert}
            label="问题"
            value={report === null ? '—' : String(report.issues.length)}
          />
        </div>

        <div className="mt-5 overflow-hidden rounded-lg border border-slate-200 bg-white shadow-sm">
          <div className="flex h-10 items-center border-b border-slate-200 px-4">
            <h2 className="text-[13px] font-semibold text-slate-800">工作流列表</h2>
            <span className="ml-auto text-[11px] text-slate-400">共 1 个</span>
          </div>
          <div className="grid grid-cols-[minmax(0,1fr)_120px_120px_36px] items-center gap-3 px-4 py-3">
            <div className="min-w-0">
              <strong className="block truncate text-[13px] font-semibold text-slate-800">
                {workflowName}
              </strong>
              <span className="mt-0.5 block text-[11px] text-slate-400">当前工作区</span>
            </div>
            <span className="text-[11px] text-slate-500">可视化流程</span>
            <span className={`justify-self-start rounded-full px-2 py-1 text-[10px] ${validationStatus.tone}`}>
              {validationStatus.label}
            </span>
            <button
              type="button"
              aria-label={`打开 ${workflowName}`}
              className="flex size-7 items-center justify-center rounded-md text-blue-600 hover:bg-blue-50"
              onClick={onOpenEditor}
              title="打开工作流"
            >
              <ArrowRight className="size-4 shrink-0" aria-hidden="true" />
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

type OverviewMetricProps = Readonly<{
  /** 指标图标。 */
  icon: LucideIcon;
  /** 指标名称。 */
  label: string;
  /** 指标值。 */
  value: string;
}>;

/** 工作流概览的紧凑统计卡片。 */
function OverviewMetric({ icon: Icon, label, value }: OverviewMetricProps) {
  return (
    <div className="flex items-center rounded-lg border border-slate-200 bg-white p-3 shadow-sm">
      <span className="flex size-9 items-center justify-center rounded-lg bg-blue-50 text-blue-600">
        <Icon className="size-4 shrink-0" aria-hidden="true" />
      </span>
      <div className="ml-3">
        <strong className="block text-lg leading-5 font-semibold text-slate-900">{value}</strong>
        <span className="text-[11px] text-slate-500">{label}</span>
      </div>
    </div>
  );
}
