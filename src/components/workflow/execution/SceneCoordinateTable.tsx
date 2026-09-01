import ChevronLeft from 'lucide-react/dist/esm/icons/chevron-left.mjs';
import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import { useEffect, useState } from 'react';

import type { SceneNodeRef, VisualQueryTrace } from '../../../features/workflow';
import { IconButton } from '../../ui';

const PAGE_SIZE = 50;

/** 大量 OCR 节点使用分页表格，避免 100+ 节点同时挂载拖慢执行台。 */
export function SceneCoordinateTable({ trace }: Readonly<{ trace: VisualQueryTrace }>) {
  const [page, setPage] = useState(0);
  const pageCount = Math.max(1, Math.ceil(trace.projection.nodes.length / PAGE_SIZE));
  useEffect(() => setPage(0), [trace.node_sequence]);
  const candidateKeys = new Set(trace.candidate_nodes.map(nodeRefKey));
  const selectedKey = trace.selected_node ? nodeRefKey(trace.selected_node) : null;
  const rows = trace.projection.nodes.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  return (
    <div className="grid h-full min-h-0 grid-rows-[minmax(0,1fr)_48px] bg-white">
      <div className="min-h-0 overflow-auto">
        <table className="w-full min-w-[980px] border-collapse text-left text-[12px]">
          <thead className="sticky top-0 z-10 bg-slate-100 text-slate-600">
            <tr>
              {['状态', '窗口', 'Scene / 帧', '文字', '屏幕坐标', '帧坐标', '置信度', '来源'].map((label) => (
                <th key={label} className="border-b border-slate-200 px-4 py-3 font-semibold">{label}</th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100">
            {rows.map((node) => {
              const key = nodeRefKey(node);
              const selected = key === selectedKey;
              const candidate = candidateKeys.has(key);
              return (
                <tr key={key} className={selected ? 'bg-emerald-50' : candidate ? 'bg-amber-50' : 'hover:bg-slate-50'}>
                  <td className="px-4 py-3 font-medium">
                    {selected ? <span className="text-emerald-700">最终命中</span>
                      : candidate ? <span className="text-amber-700">候选</span>
                        : <span className="text-slate-400">场景节点</span>}
                  </td>
                  <td className="px-4 py-3 font-mono text-slate-600">{node.window_handle}</td>
                  <td className="px-4 py-3 font-mono text-slate-600">{node.scene_id} / {node.frame_id}</td>
                  <td className="max-w-xs whitespace-normal break-words px-4 py-3 text-slate-900">{node.text}</td>
                  <td className="px-4 py-3 font-mono text-slate-600">{rectLabel(node.screen_bbox)}</td>
                  <td className="px-4 py-3 font-mono text-slate-600">{rectLabel(node.frame_bbox)}</td>
                  <td className="px-4 py-3 text-slate-600">{Math.round(node.confidence * 100)}%</td>
                  <td className="px-4 py-3 text-slate-600">{node.source}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {trace.projection.nodes.length === 0 ? (
          <p className="p-8 text-center text-[13px] text-slate-500">窗口已捕获，但没有识别到 OCR 文字节点。</p>
        ) : null}
      </div>
      <footer className="flex items-center justify-end gap-3 border-t border-slate-200 px-4 text-[12px] text-slate-500">
        <span>共 {trace.projection.nodes.length} 个节点 · 第 {page + 1} / {pageCount} 页</span>
        <IconButton icon={ChevronLeft} label="上一页坐标" disabled={page === 0} onClick={() => setPage((value) => value - 1)} />
        <IconButton icon={ChevronRight} label="下一页坐标" disabled={page >= pageCount - 1} onClick={() => setPage((value) => value + 1)} />
      </footer>
    </div>
  );
}

function nodeRefKey(node: SceneNodeRef): string {
  return `${node.window_handle}:${node.scene_id}:${node.node_id}`;
}

function rectLabel(rect: Readonly<{ x: number; y: number; width: number; height: number }>): string {
  return `${rect.x}, ${rect.y} · ${rect.width}×${rect.height}`;
}
