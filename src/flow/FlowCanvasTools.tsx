import {
  Crosshair,
  Hand,
  LockKeyhole,
  Maximize2,
  MousePointer2,
  Settings2,
  SlidersHorizontal,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';

import { useFlowStore } from './store';

type FlowCanvasToolsProps = Readonly<{
  /** 画布允许的最大放大倍率。 */
  maxZoom: number;
}>;

type CanvasToolMode = 'select' | 'pan';

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

/** 参考图中的顶部模式工具、缩略图和底部缩放工具。 */
export function FlowCanvasTools({ maxZoom }: FlowCanvasToolsProps) {
  const viewport = useFlowStore((state) => state.viewport);
  const nodes = useFlowStore((state) => state.nodes);
  const setViewport = useFlowStore((state) => state.setViewport);
  const [mode, setMode] = useState<CanvasToolMode>('select');

  const zoomOut = () => {
    setViewport({ ...viewport, zoom: Math.max(0.2, viewport.zoom * 0.8) });
  };
  const zoomIn = () => {
    setViewport({ ...viewport, zoom: Math.min(maxZoom, viewport.zoom * 1.25) });
  };
  const resetViewport = () => setViewport({ x: 0, y: 42, zoom: 1 });

  return (
    <>
      <div className="absolute top-3 right-4 z-40 flex items-center gap-2">
        <div className={TOOL_GROUP_CLASS_NAME}>
          <ToolButton
            label="选择"
            pressed={mode === 'select'}
            onClick={() => setMode('select')}
          >
            <MousePointer2 />
          </ToolButton>
          <ToolButton
            label="平移"
            pressed={mode === 'pan'}
            onClick={() => setMode('pan')}
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
      <CanvasMinimap />
      <div className="absolute right-4 bottom-3 z-40">
        <div className={TOOL_GROUP_CLASS_NAME}>
          <ToolButton label="锁定画布" onClick={() => undefined}>
            <LockKeyhole />
          </ToolButton>
          <ToolButton label="缩小" onClick={zoomOut}>
            <ZoomOut />
          </ToolButton>
          <button
            type="button"
            className="h-8 min-w-[58px] border-r border-slate-200 bg-white px-2 text-[11px] tabular-nums text-slate-600"
            onClick={resetViewport}
            title="重置为 100%"
          >
            {Math.round(viewport.zoom * 100)}%
          </button>
          <ToolButton label="放大" onClick={zoomIn}>
            <ZoomIn />
          </ToolButton>
          <ToolButton label="视图设置" onClick={() => undefined}>
            <Settings2 />
          </ToolButton>
        </div>
      </div>
      <div className="absolute right-[184px] bottom-3 z-30 flex h-8 items-center rounded-l-md border border-r-0 border-slate-300 bg-white px-3 text-[11px] text-slate-500 shadow-sm">
        <span className="mr-2 h-px w-3 bg-slate-400" />
        画布
      </div>
    </>
  );
}

/** 根据当前节点范围绘制轻量缩略图，不把业务节点类型泄漏到 Flow 内核。 */
function CanvasMinimap() {
  const nodes = useFlowStore((state) => state.nodes);
  /** 空画布使用一个稳定的 1px 边界，避免除零。 */
  const minX = nodes.length > 0 ? Math.min(...nodes.map((node) => node.position.x)) : 0;
  const minY = nodes.length > 0 ? Math.min(...nodes.map((node) => node.position.y)) : 0;
  const maxX = nodes.length > 0
    ? Math.max(...nodes.map((node) => node.position.x + node.size.width))
    : 1;
  const maxY = nodes.length > 0
    ? Math.max(...nodes.map((node) => node.position.y + node.size.height))
    : 1;
  /** 将世界坐标完整压进 132×66px 缩略图区。 */
  const scale = Math.min(132 / Math.max(1, maxX - minX), 66 / Math.max(1, maxY - minY));

  return (
    <div className="absolute right-4 bottom-[58px] z-30 h-[94px] w-[150px] rounded-md border border-slate-300 bg-white p-2 shadow-[0_2px_8px_rgba(37,53,74,.08)]">
      <div className="relative h-full w-full overflow-hidden bg-slate-50">
        {nodes.map((node, index) => (
          <span
            key={node.id}
            className={
              'absolute min-h-1 min-w-2 rounded-[1px] border ' +
              (index === 0
                ? 'border-emerald-400 bg-emerald-100'
                : 'border-blue-300 bg-blue-100')
            }
            style={{
              height: Math.max(4, node.size.height * scale),
              left: (node.position.x - minX) * scale,
              top: (node.position.y - minY) * scale,
              width: Math.max(8, node.size.width * scale),
            }}
          />
        ))}
        <span className="absolute inset-[5px] border border-blue-500 bg-blue-50/20" />
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
