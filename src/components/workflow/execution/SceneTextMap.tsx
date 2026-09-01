import Maximize2 from 'lucide-react/dist/esm/icons/maximize-2.mjs';
import Minus from 'lucide-react/dist/esm/icons/minus.mjs';
import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';

import type { SceneNodeRef, VisualQueryTrace } from '../../../features/workflow';
import { IconButton, Select } from '../../ui';

type SceneTextMapProps = Readonly<{
  trace: VisualQueryTrace;
}>;

type ViewBox = Readonly<{ x: number; y: number; width: number; height: number }>;
const WINDOW_ALL = '__all__';

/** 使用真实屏幕像素坐标绘制窗口和 OCR 文字，不再生成定宽 ASCII 投影。 */
export function SceneTextMap({ trace }: SceneTextMapProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const dragStart = useRef<Readonly<{ x: number; y: number; viewBox: ViewBox }> | null>(null);
  const [windowFilter, setWindowFilter] = useState(WINDOW_ALL);
  const windows = trace.projection.windows.filter((window) => (
    windowFilter === WINDOW_ALL || window.window_handle === windowFilter
  ));
  const nodes = trace.projection.nodes.filter((node) => (
    windowFilter === WINDOW_ALL || node.window_handle === windowFilter
  ));
  const fittedViewBox = useMemo(() => sceneBounds(windows.map((window) => window.screen_bounds)), [windows]);
  const [viewBox, setViewBox] = useState<ViewBox>(fittedViewBox);

  useEffect(() => setViewBox(fittedViewBox), [fittedViewBox]);
  const candidateKeys = new Set(trace.candidate_nodes.map(nodeRefKey));
  const selectedKey = trace.selected_node ? nodeRefKey(trace.selected_node) : null;

  const zoom = (factor: number) => setViewBox((current) => {
    const width = current.width * factor;
    const height = current.height * factor;
    return {
      x: current.x + (current.width - width) / 2,
      y: current.y + (current.height - height) / 2,
      width,
      height,
    };
  });
  const startPan = (event: ReactPointerEvent<SVGSVGElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    dragStart.current = { x: event.clientX, y: event.clientY, viewBox };
  };
  const movePan = (event: ReactPointerEvent<SVGSVGElement>) => {
    const start = dragStart.current;
    const svg = svgRef.current;
    if (!start || !svg) return;
    const bounds = svg.getBoundingClientRect();
    setViewBox({
      ...start.viewBox,
      x: start.viewBox.x - (event.clientX - start.x) * start.viewBox.width / Math.max(1, bounds.width),
      y: start.viewBox.y - (event.clientY - start.y) * start.viewBox.height / Math.max(1, bounds.height),
    });
  };

  if (trace.projection.windows.length === 0) {
    return <SceneEmpty message="本次查询没有捕获到可绘制的窗口。" />;
  }
  return (
    <div className="relative h-full min-h-0 overflow-hidden bg-slate-950">
      <div className="absolute top-4 left-4 z-10 flex items-center gap-2">
        <Select
          aria-label="场景窗口"
          density="compact"
          value={windowFilter}
          containerClassName="w-52"
          options={[
            { value: WINDOW_ALL, label: `全部窗口（${trace.projection.windows.length}）` },
            ...trace.projection.windows.map((window) => ({
              value: window.window_handle,
              label: `窗口 ${window.window_handle}${window.foreground ? ' · 前台' : ''}`,
            })),
          ]}
          onValueChange={setWindowFilter}
        />
      </div>
      <div className="absolute top-4 right-4 z-10 flex overflow-hidden rounded-md border border-slate-600 bg-slate-900 shadow-lg">
        <IconButton icon={Minus} label="缩小文字地图" onClick={() => zoom(1.25)} />
        <IconButton icon={Plus} label="放大文字地图" onClick={() => zoom(0.8)} />
        <IconButton icon={Maximize2} label="适应全部窗口" onClick={() => setViewBox(fittedViewBox)} />
      </div>
      <svg
        ref={svgRef}
        aria-label="真实坐标文字地图"
        className="h-full w-full cursor-grab touch-none select-none active:cursor-grabbing"
        viewBox={`${viewBox.x} ${viewBox.y} ${viewBox.width} ${viewBox.height}`}
        onPointerDown={startPan}
        onPointerMove={movePan}
        onPointerUp={() => { dragStart.current = null; }}
        onPointerCancel={() => { dragStart.current = null; }}
        onWheel={(event) => {
          event.preventDefault();
          zoom(event.deltaY > 0 ? 1.12 : 0.88);
        }}
      >
        {windows.slice().sort((left, right) => right.z_order - left.z_order).map((window) => (
          <g key={`${window.window_handle}-${window.scene_id}`}>
            <rect
              x={window.screen_bounds.x}
              y={window.screen_bounds.y}
              width={window.screen_bounds.width}
              height={window.screen_bounds.height}
              rx={8}
              fill={window.foreground ? '#172554' : '#111827'}
              stroke={window.foreground ? '#60a5fa' : '#475569'}
              strokeWidth={Math.max(1, viewBox.width / 1200)}
            />
            <text
              x={window.screen_bounds.x + 12}
              y={window.screen_bounds.y + 22}
              fill="#94a3b8"
              fontSize={13}
            >
              窗口 {window.window_handle} · 层级 {window.z_order}
            </text>
          </g>
        ))}
        {nodes.map((node) => {
          const key = nodeRefKey(node);
          const selected = key === selectedKey;
          const candidate = candidateKeys.has(key);
          const tone = selected ? '#34d399' : candidate ? '#fbbf24' : '#e2e8f0';
          return (
            <g key={key}>
              <rect
                x={node.screen_bbox.x}
                y={node.screen_bbox.y}
                width={node.screen_bbox.width}
                height={node.screen_bbox.height}
                fill={selected ? '#10b98133' : candidate ? '#f59e0b22' : 'transparent'}
                stroke={tone}
                strokeOpacity={selected || candidate ? 1 : 0.28}
              />
              <text
                x={node.screen_bbox.x + 2}
                y={node.screen_bbox.y + Math.min(node.screen_bbox.height - 2, 14)}
                fill={tone}
                fontSize={Math.max(10, Math.min(16, node.screen_bbox.height * 0.72))}
              >
                {node.text}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function nodeRefKey(node: SceneNodeRef): string {
  return `${node.window_handle}:${node.scene_id}:${node.node_id}`;
}

function sceneBounds(rectangles: ReadonlyArray<{ x: number; y: number; width: number; height: number }>): ViewBox {
  if (rectangles.length === 0) return { x: 0, y: 0, width: 1280, height: 720 };
  const left = Math.min(...rectangles.map((rect) => rect.x));
  const top = Math.min(...rectangles.map((rect) => rect.y));
  const right = Math.max(...rectangles.map((rect) => rect.x + rect.width));
  const bottom = Math.max(...rectangles.map((rect) => rect.y + rect.height));
  const padding = 48;
  return {
    x: left - padding,
    y: top - padding,
    width: Math.max(1, right - left + padding * 2),
    height: Math.max(1, bottom - top + padding * 2),
  };
}

function SceneEmpty({ message }: Readonly<{ message: string }>) {
  return <div className="flex h-full items-center justify-center bg-slate-950 text-[13px] text-slate-400">{message}</div>;
}
