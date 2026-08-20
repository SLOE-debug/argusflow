import type { ValidationReport } from '../../features/workflow/contracts';

type RunToolbarProps = {
  /** 当前工作流名称及其输入回写处理器。 */
  workflowName: string;
  /** 是否正在等待后端运行结果；运行中禁用重复操作。 */
  running: boolean;
  /** 最近一次启动的运行 ID；未启动时为空。 */
  runId: string | null;
  /** 最近一次结构校验结果。 */
  report: ValidationReport | null;
  /** 命令或校验失败时展示的错误消息。 */
  errorMessage: string | null;
  /** 工作流名称变化回调。 */
  onNameChange: (name: string) => void;
  /** 点击“校验”时触发的异步操作。 */
  onValidate: () => void;
  /** 点击“运行工作流”时触发的异步操作。 */
  onRun: () => void;
};

/** 顶部工作流名称、校验状态及运行操作栏。 */
export function RunToolbar({
  workflowName,
  running,
  runId,
  report,
  errorMessage,
  onNameChange,
  onValidate,
  onRun,
}: RunToolbarProps) {
  return (
    <header className="flex min-h-16 items-center gap-4 border-b border-[#1d3048] bg-[#0a1525] px-5">
      <div className="flex items-center gap-3 border-r border-[#20344d] pr-5">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-br from-sky-400 to-cyan-600 font-black text-[#06111f] shadow-[0_0_24px_rgba(56,189,248,0.2)]">
          A
        </div>
        <div>
          <div className="text-sm font-bold tracking-wide text-white">ArgusFlow</div>
          <div className="text-[10px] tracking-[0.16em] text-slate-500 uppercase">Workflow Studio</div>
        </div>
      </div>
      <input
        aria-label="工作流名称"
        value={workflowName}
        onChange={(event) => onNameChange(event.target.value)}
        className="min-w-0 flex-1 rounded-lg border border-transparent bg-transparent px-3 py-2 text-sm font-medium text-slate-200 transition hover:border-[#263d57] focus:border-sky-500/60"
      />
      <div className="hidden max-w-80 truncate text-xs text-slate-500 xl:block">
        {errorMessage ??
          (report
            ? report.valid
              ? '结构校验通过'
              : `${report.issues.length} 个结构问题`
            : runId
              ? `Run ${runId.slice(0, 8)}`
              : '内存模式')}
      </div>
      <button
        type="button"
        onClick={onValidate}
        disabled={running}
        className="rounded-lg border border-[#29415e] bg-[#122238] px-4 py-2 text-xs font-semibold text-slate-200 transition hover:border-sky-500/50 disabled:cursor-not-allowed disabled:opacity-50"
      >
        校验
      </button>
      <button
        type="button"
        onClick={onRun}
        disabled={running}
        className="rounded-lg bg-sky-400 px-5 py-2 text-xs font-bold text-[#06111f] transition hover:bg-sky-300 disabled:cursor-not-allowed disabled:bg-sky-900 disabled:text-sky-500"
      >
        {running ? '运行中…' : '运行工作流'}
      </button>
    </header>
  );
}
