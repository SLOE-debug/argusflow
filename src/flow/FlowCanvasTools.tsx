import { ZoomIn, ZoomOut } from 'lucide-react';
import type { ReactNode } from 'react';

import { useFlowStore } from './store';

type FlowCanvasToolsProps = Readonly<{
  /** 画布允许的最大放大倍率。 */
  maxZoom: number;
}>;

/** 缩放工具浮层样式。 */
const TOOL_GROUP_CLASS_NAME = [
  'flex items-center gap-0.5 rounded-[10px] border border-slate-300',
  'bg-white/95 p-1 backdrop-blur-lg',
  'shadow-[0_6px_18px_rgba(37,53,74,.1)]',
].join(' ');

/** 单个缩放按钮样式，并固定内部图标为 20px。 */
const TOOL_BUTTON_CLASS_NAME = [
  'flex size-[34px] items-center justify-center rounded-lg text-slate-600',
  'hover:bg-blue-50 hover:text-blue-600 [&>svg]:size-5',
].join(' ');

/** 渲染不显示百分比的缩放控制；排列操作由右键二级菜单承载。 */
export function FlowCanvasTools({ maxZoom }: FlowCanvasToolsProps) {
  const viewport = useFlowStore((state) => state.viewport);
  const setViewport = useFlowStore((state) => state.setViewport);

  const zoomOut = () => {
    setViewport({
      ...viewport,
      zoom: Math.max(Number.MIN_VALUE, viewport.zoom * 0.8),
    });
  };
  const zoomIn = () => {
    setViewport({
      ...viewport,
      zoom: Math.min(maxZoom, viewport.zoom * 1.25),
    });
  };

  return (
    <div className="absolute right-3.5 bottom-3 z-40 flex items-center">
      <div className={TOOL_GROUP_CLASS_NAME}>
        <ToolButton
          label="缩小"
          onClick={zoomOut}
        >
          <ZoomOut />
        </ToolButton>
        <ToolButton
          label="放大"
          onClick={zoomIn}
        >
          <ZoomIn />
        </ToolButton>
      </div>
    </div>
  );
}

type ToolButtonProps = Readonly<{
  /** 屏幕阅读器标签，同时作为鼠标悬停提示。 */
  label: string;
  /** 点击后执行的画布操作。 */
  onClick: () => void;
  /** Lucide 图标。 */
  children: ReactNode;
}>;

/** 统一画布图标按钮的可访问性属性和 20px 图标尺寸。 */
function ToolButton({ label, onClick, children }: ToolButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      className={TOOL_BUTTON_CLASS_NAME}
      onClick={onClick}
      title={label}
    >
      {children}
    </button>
  );
}
