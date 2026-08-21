import {
  BellRing,
  ChevronDown,
  ChevronUp,
  History,
  type LucideIcon,
} from 'lucide-react';
import { useState } from 'react';

import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';
import { ExecutionLog } from './ExecutionLog';
import { WorkflowTaskTable } from './WorkflowTaskTable';

type WorkflowConsolePanelProps = Readonly<{
  /** 面板是否展开。 */
  open: boolean;
  /** 当前执行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 最近一次校验结果。 */
  report: ValidationReport | null;
  /** 切换面板展开状态。 */
  onToggle: () => void;
}>;

type ConsoleTab = 'tasks' | 'runs' | 'logs' | 'alerts';

/** 底部面板页签的稳定顺序与名称。 */
const CONSOLE_TABS = [
  { id: 'tasks', label: '任务' },
  { id: 'runs', label: '运行记录' },
  { id: 'logs', label: '日志' },
  { id: 'alerts', label: '告警' },
] as const satisfies ReadonlyArray<Readonly<{ id: ConsoleTab; label: string }>>;

/** 可折叠的任务、运行与校验面板。 */
export function WorkflowConsolePanel({ open, events, report, onToggle }: WorkflowConsolePanelProps) {
  const [activeTab, setActiveTab] = useState<ConsoleTab>('tasks');
  const ToggleIcon = open ? ChevronDown : ChevronUp;

  return (
    <section className="z-[18] flex h-full min-h-0 flex-col bg-white">
      <header className="flex h-[38px] shrink-0 items-center border-b border-slate-200 bg-white px-2">
        {CONSOLE_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className={
              'relative flex h-[38px] items-center px-3 text-[12px] ' +
              (activeTab === tab.id
                ? 'font-semibold text-blue-600 after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-blue-600'
                : 'text-slate-500 hover:text-slate-800')
            }
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
        <button
          type="button"
          className="ml-auto flex size-7 items-center justify-center rounded-[4px] text-slate-600 hover:bg-slate-100"
          onClick={onToggle}
          aria-label={open ? '收起任务面板' : '展开任务面板'}
          aria-expanded={open}
        >
          <ToggleIcon className="size-3.5" aria-hidden="true" />
        </button>
      </header>
      {open ? resolveConsoleContent(activeTab, events, report) : null}
    </section>
  );
}

/** 按页签分派真实内容或明确占位页，避免多个页签共用假内容。 */
function resolveConsoleContent(
  activeTab: ConsoleTab,
  events: ReadonlyArray<ExecutionEvent>,
  report: ValidationReport | null,
) {
  switch (activeTab) {
    case 'tasks':
      return <WorkflowTaskTable />;
    case 'logs':
      return <ExecutionLog events={events} report={report} />;
    case 'runs':
      return (
        <ConsolePlaceholder
          icon={History}
          title="暂无运行记录"
          description="完整的运行实例列表将在此展示。"
        />
      );
    case 'alerts':
      return report?.issues.length ? (
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2">
            {report.issues.map((issue, index) => (
              <p
                key={`${issue.code}-${issue.node_id}-${index}`}
                className="mb-1 text-[11px] leading-5 text-rose-700 last:mb-0"
              >
                {issue.node_id ? `[${issue.node_id}] ` : ''}
                {issue.message}
              </p>
            ))}
          </div>
        </div>
      ) : (
        <ConsolePlaceholder
          icon={BellRing}
          title="暂无告警"
          description="校验问题与运行异常将在此集中显示。"
        />
      );
  }
}

type ConsolePlaceholderProps = Readonly<{
  /** 占位状态图标。 */
  icon: LucideIcon;
  /** 占位标题。 */
  title: string;
  /** 后续内容说明。 */
  description: string;
}>;

/** 底部工具页签的统一占位面板。 */
function ConsolePlaceholder({ icon: Icon, title, description }: ConsolePlaceholderProps) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center p-4">
      <div className="rounded-lg border border-dashed border-slate-300 bg-slate-50 px-8 py-5 text-center">
        <Icon className="mx-auto size-5 text-slate-400" aria-hidden="true" />
        <h3 className="mt-2 text-[12px] font-semibold text-slate-700">{title}</h3>
        <p className="mt-1 text-[11px] text-slate-500">{description}</p>
      </div>
    </div>
  );
}
