import type { ReactNode } from 'react';

import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';
import { WorkflowConsolePanel } from './WorkflowConsolePanel';

type WorkflowWorkspaceProps = Readonly<{
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

/** 中央编辑区，只负责画布和底部面板的纵向编排。 */
export function WorkflowWorkspace({
  canvas,
  open,
  events,
  report,
  onToggle,
}: WorkflowWorkspaceProps) {
  /** 编辑器不再为重复的文档页签保留额外行。 */
  const rows = open
    ? 'grid-rows-[minmax(250px,1fr)_304px]'
    : 'grid-rows-[minmax(0,1fr)_38px]';

  return (
    <section className={`grid min-h-0 min-w-0 overflow-hidden ${rows}`}>
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
