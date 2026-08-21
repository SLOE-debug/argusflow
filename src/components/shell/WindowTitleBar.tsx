import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Bell,
  ChevronDown,
  CircleHelp,
  FileText,
  Minus,
  Search,
  Square,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState, type MouseEvent } from 'react';

import appIcon from '../../assets/argusflow-icon.png';
import type { ValidationReport } from '../../features/workflow/contracts';
import { resolveWorkflowStatus } from '../workflow/workflowStatus';

type WindowTitleBarProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** 工作流是否正在运行。 */
  running: boolean;
  /** 最近一次结构校验结果。 */
  report: ValidationReport | null;
  /** 最近一次命令错误。 */
  errorMessage: string | null;
}>;

/** 自绘 Windows 标题栏按钮的公共样式。 */
const WINDOW_BUTTON_CLASS_NAME = [
  'flex h-14 w-14 items-center justify-center border-0 bg-transparent',
  'text-slate-600 outline-none hover:bg-slate-100 focus-visible:ring-2',
  'focus-visible:ring-inset focus-visible:ring-blue-500 [&>svg]:size-4',
].join(' ');

/** Windows 标题栏及参考图中的工作区、搜索和服务状态控件。 */
export function WindowTitleBar({
  workflowName,
  running,
  report,
  errorMessage,
}: WindowTitleBarProps) {
  /** 浏览器预览没有 Tauri 窗口对象；桌面端和测试替身仍返回真实句柄。 */
  const appWindow = useMemo(() => {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  }, []);
  const [maximized, setMaximized] = useState(false);
  const status = resolveWorkflowStatus(running, report, errorMessage);

  useEffect(() => {
    if (!appWindow) return undefined;

    let disposed = false;
    let stopResizeListening: (() => void) | undefined;

    /** 将系统窗口最大化状态同步到标题栏图标。 */
    const syncMaximized = async () => {
      const nextMaximized = await appWindow.isMaximized();
      if (!disposed) setMaximized(nextMaximized);
    };

    void syncMaximized();
    void appWindow.onResized(() => {
      void syncMaximized();
    }).then((stopListening) => {
      if (disposed) stopListening();
      else stopResizeListening = stopListening;
    });

    return () => {
      disposed = true;
      stopResizeListening?.();
    };
  }, [appWindow]);

  const minimize = () => void appWindow?.minimize();
  const close = () => void appWindow?.close();
  const toggleMaximized = async () => {
    if (!appWindow) return;
    await appWindow.toggleMaximize();
    setMaximized(await appWindow.isMaximized());
  };
  /** 主按键拖动标题栏，双击时切换最大化状态。 */
  const handleDragMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !appWindow) return;
    if (event.detail === 2) {
      void toggleMaximized();
      return;
    }
    void appWindow.startDragging();
  };

  return (
    <header className="z-30 flex h-14 select-none items-center border-b border-slate-200 bg-white">
      <div
        className="flex min-w-0 flex-1 items-center self-stretch pl-5"
        onMouseDown={handleDragMouseDown}
      >
        <div className="flex shrink-0 items-center">
          <span className="flex size-7 items-center justify-center overflow-hidden rounded-[5px] bg-slate-900 shadow-sm">
            <img
              className="size-5 object-contain"
              src={appIcon}
              alt=""
            />
          </span>
          <strong className="ml-2.5 text-[16px] font-semibold tracking-[-0.01em] text-slate-900">
            ArgusFlow Studio
          </strong>
        </div>
        <TitleSelect
          className="ml-8 w-[158px]"
          icon={FileText}
          label="默认工作区"
        />
        <TitleSelect
          className="ml-4 w-[160px]"
          label={workflowName}
        />
        <div className="ml-4 flex items-center gap-2 text-[12px] text-slate-500">
          <span className={`size-2 rounded-full ${status.tone}`} />
          <span>{running ? '运行中' : '已保存'}&nbsp; 10:32:45</span>
        </div>
      </div>
      <div className="flex h-14 shrink-0 items-center gap-4">
        <label className="flex h-9 w-[164px] items-center rounded-md border border-slate-200 bg-slate-50 px-3 text-slate-400 focus-within:border-blue-400 focus-within:bg-white">
          <Search className="size-4 shrink-0" aria-hidden="true" />
          <input
            aria-label="搜索"
            placeholder="搜索（⌘K）"
            className="min-w-0 flex-1 border-0 bg-transparent pl-2 text-[12px] text-slate-700 outline-none placeholder:text-slate-400"
          />
        </label>
        <button
          type="button"
          className="flex h-9 items-center gap-2 rounded-md border border-slate-200 bg-white px-3 text-[12px] text-slate-700"
        >
          <span className="size-2 rounded-full bg-emerald-600" />
          服务在线
        </button>
        <button type="button" aria-label="通知" className="text-slate-600 hover:text-slate-900">
          <Bell className="size-5" aria-hidden="true" />
        </button>
        <button type="button" aria-label="帮助" className="text-slate-600 hover:text-slate-900">
          <CircleHelp className="size-[19px]" aria-hidden="true" />
        </button>
        <span className="h-6 w-px bg-slate-200" />
      </div>
      <div className="flex h-14 shrink-0 items-center">
        <button
          type="button"
          className={WINDOW_BUTTON_CLASS_NAME}
          aria-label="最小化窗口"
          title="最小化"
          onClick={minimize}
        >
          <Minus aria-hidden="true" />
        </button>
        <button
          type="button"
          className={WINDOW_BUTTON_CLASS_NAME}
          aria-label={maximized ? '还原窗口' : '最大化窗口'}
          title={maximized ? '还原' : '最大化'}
          onClick={() => void toggleMaximized()}
        >
          {maximized ? (
            <span
              aria-hidden="true"
              className="relative size-3.5 before:absolute before:top-0 before:right-0 before:size-3 before:border before:border-current after:absolute after:bottom-0 after:left-0 after:size-3 after:border after:border-current after:bg-white"
            />
          ) : (
            <Square aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          className={`${WINDOW_BUTTON_CLASS_NAME} hover:bg-red-600 hover:text-white`}
          aria-label="关闭窗口"
          title="关闭"
          onClick={close}
        >
          <X aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}

type TitleSelectProps = Readonly<{
  /** 控件附加尺寸与间距。 */
  className: string;
  /** 当前显示值。 */
  label: string;
  /** 可选的前置图标。 */
  icon?: typeof FileText;
}>;

/** 标题栏中的 36px 下拉选择器。 */
function TitleSelect({ className, label, icon: Icon }: TitleSelectProps) {
  return (
    <button
      type="button"
      className={`flex h-9 items-center rounded-md border border-slate-200 bg-slate-50 px-3 text-[13px] text-slate-800 ${className}`}
    >
      {Icon ? <Icon className="mr-2 size-4 text-slate-600" aria-hidden="true" /> : null}
      <span className="min-w-0 flex-1 truncate text-left font-medium">{label}</span>
      <ChevronDown className="ml-2 size-3.5 shrink-0 text-slate-500" aria-hidden="true" />
    </button>
  );
}
