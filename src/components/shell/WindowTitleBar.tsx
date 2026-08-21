import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Bell,
  CircleHelp,
  House,
  Minus,
  Search,
  Square,
  Workflow,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState, type MouseEvent } from 'react';

import appIcon from '../../assets/argusflow-icon.png';
import type { ValidationReport } from '../../features/workflow/contracts';
import { Input } from '../ui';
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
  /** Home 概览是否当前可见。 */
  homeActive: boolean;
  /** 进入工作区概览。 */
  onOpenHome: () => void;
  /** 进入当前工作流编辑器。 */
  onOpenWorkflow: () => void;
}>;

/** 自绘 Windows 标题栏按钮的公共样式。 */
const WINDOW_BUTTON_CLASS_NAME = [
  'flex h-10 w-11 items-center justify-center border-0 bg-transparent',
  'text-slate-600 outline-none focus-visible:ring-2',
  'focus-visible:ring-inset focus-visible:ring-blue-500 [&>svg]:size-[13px]',
].join(' ');

/** Windows 标题栏及参考图中的工作区、搜索和服务状态控件。 */
export function WindowTitleBar({
  workflowName,
  running,
  report,
  errorMessage,
  homeActive,
  onOpenHome,
  onOpenWorkflow,
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
    const target = event.target;
    if (target instanceof HTMLElement && target.closest('button, input, select')) return;
    if (event.detail === 2) {
      void toggleMaximized();
      return;
    }
    void appWindow.startDragging();
  };

  return (
    <header className="z-30 flex h-10 select-none items-center border-b border-slate-200 bg-white">
      <div
        className="flex min-w-0 flex-1 items-center self-stretch pl-3.5"
        onMouseDown={handleDragMouseDown}
      >
        <div className="flex shrink-0 items-center">
          <span className="flex size-[22px] items-center justify-center overflow-hidden">
            <img
              className="size-[22px] object-contain"
              src={appIcon}
              alt=""
            />
          </span>
          <strong className="ml-1.5 text-[13px] font-semibold tracking-[-0.01em] text-slate-900">
            ArgusFlow Studio
          </strong>
        </div>
        <button
          type="button"
          aria-label="打开工作区概览"
          aria-current={homeActive ? 'page' : undefined}
          className={
            'ml-5 flex h-[26px] w-[138px] items-center rounded-md border px-2 ' +
            'text-[12px] leading-none outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ' +
            (homeActive
              ? 'border-blue-300 bg-blue-50 text-blue-700'
              : 'border-slate-200 bg-slate-50 text-slate-700 hover:border-blue-200 hover:bg-white')
          }
          onClick={onOpenHome}
          title="工作区概览"
        >
          <House className="size-3 shrink-0" aria-hidden="true" />
          <span className="ml-1.5 truncate">默认工作区</span>
        </button>
        <button
          type="button"
          aria-label={`打开工作流 ${workflowName}`}
          aria-current={homeActive ? undefined : 'page'}
          className={
            'ml-2.5 flex h-[26px] w-[140px] items-center rounded-md border px-2 ' +
            'text-[12px] leading-none outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ' +
            (homeActive
              ? 'border-slate-200 bg-slate-50 text-slate-700 hover:border-blue-200 hover:bg-white'
              : 'border-blue-300 bg-blue-50 text-blue-700')
          }
          onClick={onOpenWorkflow}
          title={`打开 ${workflowName}`}
        >
          <Workflow className="size-3 shrink-0" aria-hidden="true" />
          <span className="ml-1.5 truncate">{workflowName}</span>
        </button>
        <div className="ml-2.5 flex h-[26px] items-center gap-1.5 text-[11px] text-slate-500">
          <span className={`size-1.5 shrink-0 rounded-full ${status.tone}`} />
          <span className="flex h-full items-center leading-none">
            {running ? '运行中' : '已保存'}&nbsp; 10:32:45
          </span>
        </div>
      </div>
      <div className="flex h-10 shrink-0 items-center gap-2.5">
        <Input
          aria-label="搜索"
          density="compact"
          containerClassName="w-[144px]"
          placeholder="搜索（⌘K）"
          startAdornment={(
            <Search
              className="size-3 shrink-0"
              aria-hidden="true"
            />
          )}
        />
        <button
          type="button"
          className="flex h-[26px] items-center gap-1.5 rounded-md border border-slate-200 bg-white px-2 text-[12px] leading-none text-slate-700"
        >
          <span className="size-1.5 rounded-full bg-emerald-600" />
          服务在线
        </button>
        <button type="button" aria-label="通知" className="text-slate-600 hover:text-slate-900">
          <Bell className="size-[15px]" aria-hidden="true" />
        </button>
        <button type="button" aria-label="帮助" className="text-slate-600 hover:text-slate-900">
          <CircleHelp className="size-[15px]" aria-hidden="true" />
        </button>
        <span className="h-4 w-px bg-slate-200" />
      </div>
      <div className="flex h-10 shrink-0 items-center">
        <button
          type="button"
          className={`${WINDOW_BUTTON_CLASS_NAME} hover:bg-slate-100`}
          aria-label="最小化窗口"
          title="最小化"
          onClick={minimize}
        >
          <Minus aria-hidden="true" />
        </button>
        <button
          type="button"
          className={`${WINDOW_BUTTON_CLASS_NAME} hover:bg-slate-100`}
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
