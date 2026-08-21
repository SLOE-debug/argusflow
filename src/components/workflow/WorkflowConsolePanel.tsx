import { ChevronDown, ChevronUp } from 'lucide-react';
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

/** 可折叠的任务、运行与校验面板。 */
export function WorkflowConsolePanel({ open, events, report, onToggle }: WorkflowConsolePanelProps) {
  const [activeTab, setActiveTab] = useState<ConsoleTab>('tasks');
  const ToggleIcon = open ? ChevronDown : ChevronUp;
  const tabs = [
    { id: 'tasks', label: '任务' },
    { id: 'runs', label: '运行记录' },
    { id: 'logs', label: '日志' },
    { id: 'alerts', label: '告警' },
  ] as const satisfies ReadonlyArray<Readonly<{ id: ConsoleTab; label: string }>>;

  return (
    <section className="z-[18] flex h-full min-h-0 flex-col bg-white">
      <header className="flex h-[38px] shrink-0 items-center border-b border-slate-200 bg-white px-2">
        {tabs.map((tab) => (
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
      {open ? (
        activeTab === 'tasks' ? (
          <WorkflowTaskTable />
        ) : (
          <ExecutionLog
            events={events}
            report={report}
          />
        )
      ) : null}
    </section>
  );
}
