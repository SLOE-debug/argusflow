import {
  Crosshair,
  Hand,
  Maximize2,
  MousePointer2,
  SlidersHorizontal,
} from 'lucide-react';
import type {
  PointerEvent as ReactPointerEvent,
  ReactNode,
} from 'react';

import { useFlowStore } from './store';

/** 画布指针工具的互斥模式。 */
export type CanvasToolMode = 'select' | 'pan';

type FlowCanvasToolsProps = Readonly<{
  /** 当前画布指针工具。 */
  mode: CanvasToolMode;
  /** 请求切换画布指针工具。 */
  onModeChange: (mode: CanvasToolMode) => void;
}>;

/** 浮动工具组的统一外观。 */
const TOOL_GROUP_CLASS_NAME = [
  'flex items-center overflow-hidden rounded-md border border-slate-300',
  'bg-white shadow-[0_2px_8px_rgba(37,53,74,.10)]',
].join(' ');

/** 单个浮动工具按钮的统一尺寸。 */
const TOOL_BUTTON_CLASS_NAME = [
  'flex size-8 items-center justify-center border-r border-slate-200 text-slate-600',
  'outline-none last:border-r-0 hover:bg-slate-50 hover:text-slate-900',
  'focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-500 [&>svg]:size-4',
].join(' ');

/** 画布右上角的模式和视口工具。 */
export function FlowCanvasTools({ mode, onModeChange }: FlowCanvasToolsProps) {
  const setViewport = useFlowStore((state) => state.setViewport);

  const resetViewport = () => setViewport({ x: 0, y: 42, zoom: 1 });
  /** 工具栏本身不得触发画布框选或平移手势。 */
  const stopCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.stopPropagation();
  };

  return (
    <div
      className="absolute top-3 right-4 z-40 flex items-center gap-2"
      onPointerDown={stopCanvasGesture}
    >
      <div className={TOOL_GROUP_CLASS_NAME}>
        <ToolButton
          label="选择"
          pressed={mode === 'select'}
          onClick={() => onModeChange('select')}
        >
          <MousePointer2 />
        </ToolButton>
        <ToolButton
          label="平移"
          pressed={mode === 'pan'}
          onClick={() => onModeChange('pan')}
        >
          <Hand />
        </ToolButton>
      </div>
      <div className={TOOL_GROUP_CLASS_NAME}>
        <ToolButton label="居中画布" onClick={resetViewport}>
          <Crosshair />
        </ToolButton>
        <ToolButton label="适应内容" onClick={resetViewport}>
          <Maximize2 />
        </ToolButton>
        <ToolButton label="画布设置" onClick={() => undefined}>
          <SlidersHorizontal />
        </ToolButton>
      </div>
    </div>
  );
}

type ToolButtonProps = Readonly<{
  /** 屏幕阅读器标签，同时作为鼠标悬停提示。 */
  label: string;
  /** 工具是否处于选中状态。 */
  pressed?: boolean;
  /** 点击后执行的画布操作。 */
  onClick: () => void;
  /** Lucide 图标。 */
  children: ReactNode;
}>;

/** 统一画布图标按钮的可访问性属性和状态样式。 */
function ToolButton({ label, pressed = false, onClick, children }: ToolButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      className={`${TOOL_BUTTON_CLASS_NAME} ${pressed ? 'bg-blue-50 text-blue-600' : ''}`}
      onClick={onClick}
      title={label}
    >
      {children}
    </button>
  );
}
