import { useMemo, useState } from 'react';

import type { VisualQueryTrace } from '../../../features/workflow';
import { Button } from '../../ui';

type SceneQueryInspectorProps = Readonly<{
  trace: VisualQueryTrace | null;
}>;

type InspectorTab = 'layout' | 'coordinates' | 'query';

const PAGE_SIZE = 100;

/** 展示 OCR Scene 的近似布局、精确坐标事实与 AQL 热路径计数器。 */
export function SceneQueryInspector({ trace }: SceneQueryInspectorProps) {
  const [tab, setTab] = useState<InspectorTab>('layout');
  const [page, setPage] = useState(0);
  const pageCount = Math.max(1, Math.ceil((trace?.projection.nodes.length ?? 0) / PAGE_SIZE));
  const visibleNodes = useMemo(() => trace?.projection.nodes.slice(
    page * PAGE_SIZE,
    (page + 1) * PAGE_SIZE,
  ) ?? [], [page, trace]);

  if (!trace) {
    return (
      <p className="mb-2 rounded-md border border-dashed border-slate-300 bg-slate-50 p-3 text-[11px] text-slate-500">
        本次查询没有保存场景数据。
      </p>
    );
  }

  return (
    <section className="mb-2 rounded-md border border-slate-200 bg-white p-2">
      <div
        className="mb-2 flex items-center gap-1"
        role="tablist"
        aria-label="OCR 场景查询详情"
      >
        {TABS.map((item) => (
          <Button
            key={item.value}
            variant={tab === item.value ? 'secondary' : 'ghost'}
            size="compact"
            role="tab"
            aria-selected={tab === item.value}
            onClick={() => setTab(item.value)}
          >
            {item.label}
          </Button>
        ))}
      </div>
      {tab === 'layout' ? (
        <div role="tabpanel">
          <p className="mb-1 text-[10px] text-slate-500">
            布局用于查看大致位置；点击坐标来自精确边界。
          </p>
          <pre className="max-h-72 overflow-auto rounded bg-slate-950 p-2 font-mono text-[10px] leading-4 text-slate-100">
            {trace.projection.nodes.length > 0
              ? trace.projection.nodes.map((node) => (
                  `${node.window_handle} · ${node.screen_bbox.x},${node.screen_bbox.y} · ${node.text}`
                )).join('\n')
              : '本次查询没有保存场景数据。'}
          </pre>
        </div>
      ) : null}
      {tab === 'coordinates' ? (
        <div role="tabpanel" className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-[10px]">
            <thead className="text-slate-500">
              <tr>
                <th className="border-b border-slate-200 p-1">文字</th>
                <th className="border-b border-slate-200 p-1">窗口</th>
                <th className="border-b border-slate-200 p-1">屏幕 BBox</th>
                <th className="border-b border-slate-200 p-1">置信度</th>
              </tr>
            </thead>
            <tbody>
              {visibleNodes.map((node) => (
                <tr key={`${node.scene_id}-${node.node_id}`} className="align-top text-slate-700">
                  <td className="max-w-40 border-b border-slate-100 p-1 break-all">{node.text}</td>
                  <td className="border-b border-slate-100 p-1 font-mono">{node.window_handle}</td>
                  <td className="border-b border-slate-100 p-1 font-mono">
                    {node.screen_bbox.x}, {node.screen_bbox.y}, {node.screen_bbox.width} × {node.screen_bbox.height}
                  </td>
                  <td className="border-b border-slate-100 p-1 font-mono">
                    {(node.confidence * 100).toFixed(1)}%
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="mt-1 flex items-center justify-end gap-1 text-[10px] text-slate-500">
            <Button
              variant="ghost"
              size="compact"
              disabled={page === 0}
              aria-label="上一页文字坐标"
              onClick={() => setPage((current) => Math.max(0, current - 1))}
            >
              上一页
            </Button>
            <span>{page + 1} / {pageCount}</span>
            <Button
              variant="ghost"
              size="compact"
              disabled={page + 1 >= pageCount}
              aria-label="下一页文字坐标"
              onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
            >
              下一页
            </Button>
          </div>
        </div>
      ) : null}
      {tab === 'query' ? (
        <dl role="tabpanel" className="grid grid-cols-[92px_minmax(0,1fr)] gap-x-2 gap-y-1 text-[10px]">
          <dt className="text-slate-400">AQL</dt>
          <dd className="select-text break-all font-mono text-slate-700">{trace.query}</dd>
          <dt className="text-slate-400">结果</dt>
          <dd className="text-slate-700">{OUTCOME_LABELS[trace.outcome]}</dd>
          <dt className="text-slate-400">耗时</dt>
          <dd className="font-mono text-slate-700">{trace.metrics.elapsed_us} μs</dd>
          <dt className="text-slate-400">索引命中</dt>
          <dd className="font-mono text-slate-700">{trace.metrics.exact_index_hits}</dd>
          <dt className="text-slate-400">扫描节点</dt>
          <dd className="font-mono text-slate-700">{trace.metrics.scanned_nodes}</dd>
          <dt className="text-slate-400">空间候选</dt>
          <dd className="font-mono text-slate-700">{trace.metrics.spatial_candidates}</dd>
        </dl>
      ) : null}
    </section>
  );
}

const TABS = [
  { value: 'layout', label: '场景布局' },
  { value: 'coordinates', label: '文字坐标' },
  { value: 'query', label: '查询过程' },
] as const satisfies ReadonlyArray<Readonly<{ value: InspectorTab; label: string }>>;

const OUTCOME_LABELS = {
  not_found: '未找到',
  unique: '唯一命中',
  multiple: '多项结果',
  ambiguous: '结果不唯一',
  rejected_confidence: '置信度不足',
} satisfies Readonly<Record<VisualQueryTrace['outcome'], string>>;
