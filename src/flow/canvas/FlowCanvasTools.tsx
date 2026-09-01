import Crosshair from 'lucide-react/dist/esm/icons/crosshair.mjs';
import Hand from 'lucide-react/dist/esm/icons/hand.mjs';
import Maximize2 from 'lucide-react/dist/esm/icons/maximize-2.mjs';
import MousePointer2 from 'lucide-react/dist/esm/icons/mouse-pointer-2.mjs';
import SlidersHorizontal from 'lucide-react/dist/esm/icons/sliders-horizontal.mjs';
import type {
  PointerEvent as ReactPointerEvent,
  ReactNode,
} from 'react';
import { useState } from 'react';
import type { FlowCanvasInteractionMode } from './FlowCanvas';

import { useFlowStore } from '../store/store';
import {
  MAX_CANVAS_ZOOM,
  centerBoundsInViewport,
  fitBoundsToViewport,
  getNodesBounds,
} from '../viewport/viewport';

/** 画布指针工具的互斥模式。 */
export type CanvasToolMode = 'select' | 'pan';

type FlowCanvasToolsProps = Readonly<{
  /** 当前可见画布的屏幕尺寸。 */
  canvasSize: Readonly<{ width: number; height: number }>;
  /** 当前画布指针工具。 */
  mode: CanvasToolMode;
  /** 请求切换画布指针工具。 */
  onModeChange: (mode: CanvasToolMode) => void;
  interactionMode?: FlowCanvasInteractionMode;
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
  'focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-500 [&>svg]:size-4 [&>svg]:shrink-0',
].join(' ');

/** 画布右上角的模式和视口工具。 */
export function FlowCanvasTools({
  canvasSize,
  mode,
  onModeChange,
  interactionMode = 'editable',
}: FlowCanvasToolsProps) {
  const nodes = useFlowStore((state) => state.nodes);
  const selectedNodeIds = useFlowStore((state) => state.selectedNodeIds);
  const viewport = useFlowStore((state) => state.viewport);
  const setViewport = useFlowStore((state) => state.setViewport);
  const [settingsOpen, setSettingsOpen] = useState(false);

  /** 优先定位选中节点；没有选择时定位全部内容，并保持当前缩放。 */
  const locate = () => {
    const selectedNodes = nodes.filter((node) => selectedNodeIds.has(node.id));
    const bounds = getNodesBounds(selectedNodes.length > 0 ? selectedNodes : nodes);
    if (!bounds) return;

    setViewport(centerBoundsInViewport(bounds, canvasSize, viewport.zoom));
  };
  /** 自动计算缩放和平移，使全部节点完整进入当前画布。 */
  const fitContent = () => {
    const bounds = getNodesBounds(nodes);
    if (!bounds) return;

    setViewport(fitBoundsToViewport(bounds, canvasSize, {
      padding: 72,
      maxZoom: MAX_CANVAS_ZOOM,
    }));
  };
  /** 工具栏本身不得触发画布框选或平移手势。 */
  const stopCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.stopPropagation();
  };

  return (
    <div
      className="absolute top-3 right-6 z-40 flex items-center gap-2"
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
        <ToolButton label="居中显示" onClick={locate}>
          <Crosshair />
        </ToolButton>
        <ToolButton label="显示全部" onClick={fitContent}>
          <Maximize2 />
        </ToolButton>
        {interactionMode === 'editable' ? (
          <ToolButton
            label="画布设置"
            pressed={settingsOpen}
            onClick={() => setSettingsOpen((open) => !open)}
          >
            <SlidersHorizontal />
          </ToolButton>
        ) : null}
      </div>
      {interactionMode === 'editable' && settingsOpen ? <CanvasShortcutPanel /> : null}
    </div>
  );
}

/** 画布设置入口对应的快捷操作说明面板。 */
function CanvasShortcutPanel() {
  return (
    <div className="absolute top-10 right-0 w-56 rounded-md border border-slate-200 bg-white p-3 text-[11px] text-slate-600 shadow-lg">
      <h3 className="text-[12px] font-semibold text-slate-800">画布快捷操作</h3>
      <div className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-2">
        <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px]">方向键</kbd>
        <span>移动选中节点 1 像素</span>
        <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px]">Shift + 方向键</kbd>
        <span>快速移动 10 像素</span>
        <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px]">空格 + 拖拽</kbd>
        <span>平移画布</span>
        <kbd className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px]">滚轮</kbd>
        <span>缩放画布</span>
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
