import { type ReactNode } from 'react';
import { PanelBottom, PanelLeft, PanelRight, Play } from 'lucide-react';

import appIcon from '../../assets/argusflow-icon.png';
import type { ValidationReport } from '../../features/workflow/contracts';

type RunToolbarProps = {
  running: boolean;
  report: ValidationReport | null;
  errorMessage: string | null;
  onValidate: () => void;
  onRun: () => void;
  onToggleLibrary: () => void;
  onToggleInspector: () => void;
  onToggleConsole: () => void;
};

type ToolbarIconButtonProps = {
  /** 图标按钮的可访问性标签。 */
  label: string;
  /** 点击时切换对应面板。 */
  onClick: () => void;
  /** 20px Lucide 图标。 */
  children: ReactNode;
};

type ToolbarStatus = {
  readonly label: string;
  readonly tone: string;
};

/** IDE 顶部命令栏，不在顶部占用空间编辑工作流名称。 */
export function RunToolbar({
  running,
  report,
  errorMessage,
  onValidate,
  onRun,
  onToggleLibrary,
  onToggleInspector,
  onToggleConsole,
}: RunToolbarProps) {
  const status = resolveToolbarStatus(report, errorMessage);

  return (
    <header
      className={
        'z-20 flex items-center gap-3 border-b border-slate-300/80 bg-white/95 ' +
        'px-3 shadow-[0_3px_14px_rgba(30,48,74,.05)] backdrop-blur-xl'
      }
    >
      <div className="flex w-[218px] items-center gap-2.5">
        <img
          className="size-8 object-contain drop-shadow-[0_3px_6px_rgba(37,99,235,.18)]"
          src={appIcon}
          alt=""
        />
        <div className="flex min-w-0 flex-col justify-center">
          <strong className="text-base leading-tight">ArgusFlow</strong>
          <span
            className={
              'mt-0.5 text-[10px] leading-none font-bold tracking-[.14em] ' +
              'text-slate-500'
            }
          >
            WORKFLOW STUDIO
          </span>
        </div>
      </div>
      <div className="flex items-center gap-0.5 border-l border-slate-300 pl-2.5">
        <ToolbarIconButton
          label="切换节点库"
          onClick={onToggleLibrary}
        >
          <PanelLeft />
        </ToolbarIconButton>
        <ToolbarIconButton
          label="切换属性栏"
          onClick={onToggleInspector}
        >
          <PanelRight />
        </ToolbarIconButton>
        <ToolbarIconButton
          label="切换运行面板"
          onClick={onToggleConsole}
        >
          <PanelBottom />
        </ToolbarIconButton>
      </div>
      <div className="ml-auto flex items-center gap-2 text-[13px] text-slate-600">
        <span className={`size-2 rounded-full ${status.tone}`} />
        {status.label}
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          className={
            'flex h-[34px] items-center justify-center rounded-lg border border-slate-300 ' +
            'bg-white px-3 text-[13px] font-bold text-slate-600 hover:bg-slate-50 ' +
            'disabled:cursor-not-allowed disabled:opacity-45'
          }
          onClick={onValidate}
          disabled={running}
        >
          校验
        </button>
        <button
          type="button"
          className={
            'flex h-[34px] items-center justify-center gap-2 rounded-lg border ' +
            'border-blue-700 bg-gradient-to-br from-blue-500 to-blue-600 px-3 ' +
            'text-[13px] font-bold text-white shadow-[0_4px_10px_rgba(37,99,235,.2)] ' +
            'hover:from-blue-600 hover:to-blue-700 disabled:cursor-not-allowed disabled:opacity-45'
          }
          onClick={onRun}
          disabled={running}
        >
          {running ? '运行中…' : '运行工作流'}
          <Play
            className="size-5 fill-current"
            aria-hidden="true"
          />
        </button>
      </div>
    </header>
  );
}

/** 将校验及运行错误归一为工具栏展示状态。 */
function resolveToolbarStatus(
  report: ValidationReport | null,
  errorMessage: string | null,
): ToolbarStatus {
  if (errorMessage) {
    return {
      label: '发生错误',
      tone: 'bg-rose-500 ring-4 ring-rose-100',
    };
  }

  if (report?.valid) {
    return {
      label: '校验通过',
      tone: 'bg-emerald-500 ring-4 ring-emerald-100',
    };
  }

  if (report) {
    return {
      label: `${report.issues.length} 个问题`,
      tone: 'bg-slate-400',
    };
  }

  return {
    label: '就绪',
    tone: 'bg-slate-400',
  };
}

/** 顶部工具栏的统一 20px 图标按钮。 */
function ToolbarIconButton({ label, onClick, children }: ToolbarIconButtonProps) {
  return (
    <button
      type="button"
      className={
        'flex size-[34px] items-center justify-center rounded-lg text-slate-600 ' +
        'hover:bg-blue-50 hover:text-blue-600'
      }
      onClick={onClick}
      title={label}
      aria-label={label}
    >
      <span className="flex size-5 items-center justify-center [&>svg]:size-5">
        {children}
      </span>
    </button>
  );
}
