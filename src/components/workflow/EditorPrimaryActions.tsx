import {
  ChevronDown,
  Play,
  ShieldCheck,
  Upload,
  type LucideIcon,
} from 'lucide-react';

type EditorPrimaryActionsProps = Readonly<{
  /** 后端运行是否正在进行。 */
  running: boolean;
  /** 请求结构校验。 */
  onValidate: () => void;
  /** 请求运行当前工作流。 */
  onRun: () => void;
  /** 请求发布当前工作流。 */
  onPublish: () => void;
}>;

/** 标题栏中的校验、运行和发布主操作。 */
export function EditorPrimaryActions({
  running,
  onValidate,
  onRun,
  onPublish,
}: EditorPrimaryActionsProps) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <button
        type="button"
        className="hidden h-[26px] items-center gap-1.5 rounded-md border border-slate-300 bg-white px-2.5 text-[12px] leading-none font-medium text-slate-700 outline-none hover:bg-slate-50 focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-40 min-[1100px]:flex"
        onClick={onValidate}
        disabled={running}
        aria-label="校验"
      >
        <ShieldCheck className="size-3" aria-hidden="true" />
        校验
      </button>
      <SplitActionButton
        label={running ? '运行中…' : '运行'}
        icon={Play}
        disabled={running}
        onClick={onRun}
      />
      <span className="hidden min-[1360px]:block">
        <SplitActionButton
          label="发布"
          icon={Upload}
          onClick={onPublish}
        />
      </span>
    </div>
  );
}

type SplitActionButtonProps = Readonly<{
  /** 主按钮文字，同时作为可访问名称。 */
  label: string;
  /** 主按钮 Lucide 图标。 */
  icon: LucideIcon;
  /** 当前是否不可用。 */
  disabled?: boolean;
  /** 主操作回调。 */
  onClick: () => void;
}>;

/** 右侧带保留选项入口的蓝色主操作。 */
function SplitActionButton({
  label,
  icon: Icon,
  disabled = false,
  onClick,
}: SplitActionButtonProps) {
  return (
    <div className="flex h-[26px] overflow-hidden rounded-md bg-blue-600 text-white shadow-sm">
      <button
        type="button"
        className="flex h-[26px] items-center gap-1.5 px-2.5 text-[12px] leading-none font-semibold outline-none hover:bg-blue-700 disabled:cursor-default disabled:opacity-45"
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
      >
        <Icon className="size-3" aria-hidden="true" />
        {label}
      </button>
      <button
        type="button"
        aria-label={`${label}选项`}
        disabled={disabled}
        className="flex h-[26px] w-[26px] items-center justify-center border-l border-blue-500 hover:bg-blue-700 disabled:opacity-45"
      >
        <ChevronDown className="size-2.5" aria-hidden="true" />
      </button>
    </div>
  );
}
