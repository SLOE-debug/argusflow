import { X } from 'lucide-react';
import type { ReactNode } from 'react';

import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';
import { WorkflowConsolePanel } from './WorkflowConsolePanel';

type WorkflowWorkspaceProps = Readonly<{
  /** 当前文档名称。 */
  workflowName: string;
  /** 已装配 Flow Provider 的画布。 */
  canvas: ReactNode;
  /** 底部任务面板是否展开。 */
  open: boolean;
  /** 当前运行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 最近一次结构校验结果。 */
  report: ValidationReport | null;
  /** 切换任务面板。 */
  onToggle: () => void;
}>;

/** 中央工作区，只负责文档页签、画布和底部面板的纵向编排。 */
export function WorkflowWorkspace({
  workflowName,
  canvas,
  open,
  events,
  report,
  onToggle,
}: WorkflowWorkspaceProps) {
  /** 展开时严格保留参考图中的 304px 任务区域。 */
  const rows = open
    ? 'grid-rows-[40px_minmax(250px,1fr)_304px]'
    : 'grid-rows-[40px_minmax(0,1fr)_38px]';

  return (
    <section className={`grid min-h-0 min-w-0 overflow-hidden ${rows}`}>
      <header className="flex h-10 items-end border-b border-slate-200 bg-slate-50">
        <div
          className={
            'flex h-10 min-w-[150px] items-center gap-3 border-r border-t-2 ' +
            'border-r-slate-200 border-t-blue-600 bg-white px-4 text-[13px]'
          }
        >
          <span className="font-semibold text-blue-600">{workflowName}</span>
          <X
            className="ml-auto size-3.5 text-slate-500"
            aria-hidden="true"
          />
        </div>
      </header>
      <div className="relative min-h-0 min-w-0 overflow-hidden border-b border-slate-200">
        {canvas}
      </div>
      <WorkflowConsolePanel
        open={open}
        events={events}
        report={report}
        onToggle={onToggle}
      />
    </section>
  );
}
