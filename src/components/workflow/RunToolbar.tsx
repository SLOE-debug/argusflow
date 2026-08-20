import type { ValidationReport } from '../../features/workflow/contracts';
import appIcon from '../../assets/argusflow-icon.png';

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
    <header className="argus-toolbar flex min-h-18 items-center gap-5 border-b px-6">
      <div className="argus-divider flex items-center gap-3.5 border-r pr-6">
        <img
          src={appIcon}
          alt=""
          className="argus-brand-icon h-12 w-12 shrink-0 object-contain"
        />
        <div>
          <div className="argus-brand-title font-bold tracking-wide">
            ArgusFlow
          </div>
          <div className="argus-muted text-xs tracking-[0.14em] uppercase">Workflow Studio</div>
        </div>
      </div>
      <input
        aria-label="工作流名称"
        value={workflowName}
        onChange={(event) => onNameChange(event.target.value)}
        className="argus-input min-w-0 flex-1 px-3.5 py-2.5 text-sm font-semibold"
      />
      <div className="argus-muted hidden max-w-80 truncate text-xs xl:block">
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
        className="argus-button-secondary px-4 py-2.5 text-sm font-semibold"
      >
        校验
      </button>
      <button
        type="button"
        onClick={onRun}
        disabled={running}
        className="argus-button-primary px-5 py-2.5 text-sm font-bold"
      >
        {running ? '运行中…' : '运行工作流'}
      </button>
    </header>
  );
}
