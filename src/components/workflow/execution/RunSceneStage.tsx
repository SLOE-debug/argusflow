import ScanSearch from 'lucide-react/dist/esm/icons/scan-search.mjs';
import ImageOff from 'lucide-react/dist/esm/icons/image-off.mjs';
import { useState } from 'react';

import {
  selectRunSceneTrace,
  type RunDetails,
  type VisualQueryTrace,
} from '../../../features/workflow';
import { SceneCoordinateTable } from './SceneCoordinateTable';
import { SceneScreenshotStage } from './SceneScreenshotStage';
import { SceneTextMap } from './SceneTextMap';

type SceneTab = 'text_map' | 'screenshot' | 'coordinates';

type RunSceneStageProps = Readonly<{
  details: RunDetails | null;
  selectedNodeId: string | null;
  selectedNodeSequence: number | null;
  cursorSequence: number | null;
  sceneInvalidatedAtSequence: number;
}>;

const SCENE_TABS = [
  { id: 'text_map', label: '文字地图' },
  { id: 'screenshot', label: '截图标注' },
  { id: 'coordinates', label: '坐标' },
] as const satisfies ReadonlyArray<{ id: SceneTab; label: string }>;

/** 场景页只有顶部摘要和单一全幅舞台，默认打开真实坐标文字地图。 */
export function RunSceneStage({
  details,
  selectedNodeId,
  selectedNodeSequence,
  cursorSequence,
  sceneInvalidatedAtSequence,
}: RunSceneStageProps) {
  const [tab, setTab] = useState<SceneTab>('text_map');
  const selection = selectRunSceneTrace(
    details,
    cursorSequence,
    selectedNodeId,
    selectedNodeSequence,
    sceneInvalidatedAtSequence,
  );
  const trace = selection.trace;
  return (
    <div className="grid h-full min-h-0 grid-rows-[52px_minmax(0,1fr)]">
      <header className="flex items-center gap-5 border-b border-slate-200 bg-white px-5">
        <div className="flex items-center gap-1 self-stretch" role="tablist" aria-label="场景视图">
          {SCENE_TABS.map((item) => (
            // 场景页签是主舞台的业务视图切换，不承担通用按钮语义。
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              aria-controls="run-scene-panel"
              className={
                'relative h-full px-3 text-[13px] font-medium ' +
                (tab === item.id
                  ? 'text-blue-700 after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-blue-600'
                  : 'text-slate-500 hover:text-slate-900')
              }
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <div className="min-w-0 flex-1 truncate text-[12px] text-slate-500">
          {trace ? (
            <>
              <span className="font-medium text-slate-800">{trace.query}</span>
              <span className="mx-2">·</span>
              <span>{outcomeLabel(trace)}</span>
              <span className="mx-2">·</span>
              <span>{trace.candidate_nodes.length} 个候选</span>
              <span className="mx-2">·</span>
              <span>{formatElapsed(trace.metrics.elapsed_us)}</span>
              <span className="mx-2">·</span>
              <span>采集于事件 #{selection.capturedAtSequence}</span>
            </>
          ) : '当前事件没有场景证据'}
        </div>
      </header>
      <div
        id="run-scene-panel"
        role="tabpanel"
        className="min-h-0 overflow-hidden"
      >
        {trace ? (
          <>
            {tab === 'text_map' ? <SceneTextMap trace={trace} /> : null}
            {tab === 'screenshot' ? (
              <SceneScreenshotStage
                runId={details?.manifest.run_id ?? trace.run_id}
                trace={trace}
                artifacts={details?.artifacts ?? []}
              />
            ) : null}
            {tab === 'coordinates' ? <SceneCoordinateTable trace={trace} /> : null}
          </>
        ) : <EmptySceneTab tab={tab} />}
      </div>
    </div>
  );
}

/** 当前回放事件缺少场景证据时，仍保留用户选中的场景视图。 */
function EmptySceneTab({ tab }: Readonly<{ tab: SceneTab }>) {
  const screenshotSelected = tab === 'screenshot';
  const title = screenshotSelected
    ? '当前事件没有截图'
    : tab === 'coordinates'
      ? '当前事件没有坐标数据'
      : '当前事件没有文字地图';
  const Icon = screenshotSelected ? ImageOff : ScanSearch;
  return (
    <div className="flex h-full items-center justify-center bg-slate-50 p-8 text-center">
      <div className="max-w-md">
        <Icon className="mx-auto size-9 text-slate-300" aria-hidden="true" />
        <h2 className="mt-3 text-[15px] font-semibold text-slate-700">{title}</h2>
        <p className="mt-2 text-[13px] leading-6 text-slate-500">
          切换时间点时会保留当前视图；请选择包含检查结果的事件。
          如果这次运行没有场景证据，请重新运行后查看。
        </p>
      </div>
    </div>
  );
}

function outcomeLabel(trace: VisualQueryTrace): string {
  switch (trace.outcome) {
    case 'not_found': return '没有候选';
    case 'unique': return '已命中唯一目标';
    case 'multiple': return '返回多个目标';
    case 'ambiguous': return '候选无法安全区分';
    case 'rejected_confidence': return '候选置信度不足';
  }
}

function formatElapsed(microseconds: number): string {
  return microseconds >= 1_000
    ? `${(microseconds / 1_000).toFixed(1)} ms`
    : `${microseconds} μs`;
}
