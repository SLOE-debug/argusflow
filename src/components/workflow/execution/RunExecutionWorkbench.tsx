import History from 'lucide-react/dist/esm/icons/history.mjs';
import RefreshCw from 'lucide-react/dist/esm/icons/refresh-cw.mjs';
import X from 'lucide-react/dist/esm/icons/x.mjs';
import { useEffect, useRef } from 'react';

import {
  useRunExecutionWorkbench,
  type ExecutionEvent,
  type LiveRunSnapshot,
  type RunWorkbenchSource,
  type RunWorkbenchView,
} from '../../../features/workflow';
import { Button, IconButton, Select } from '../../ui';
import { RunDataStage } from './RunDataStage';
import { RunFlowStage } from './RunFlowStage';
import { RunPlaybackTransport } from './RunPlaybackTransport';
import { RunSceneStage } from './RunSceneStage';

type RunExecutionWorkbenchProps = Readonly<{
  initialSource: RunWorkbenchSource;
  liveEvents: ReadonlyArray<ExecutionEvent>;
  liveRunId: string | null;
  liveSnapshot: LiveRunSnapshot | null;
  onClose: () => void;
}>;

const MAIN_VIEWS = [
  { id: 'flow', label: '流程' },
  { id: 'scene', label: '场景' },
  { id: 'data', label: '数据' },
] as const satisfies ReadonlyArray<{ id: RunWorkbenchView; label: string }>;

/** 覆盖编辑器内容区的沉浸式运行执行台，顶部应用栏与底部状态栏由 App 保留。 */
export function RunExecutionWorkbench({
  initialSource,
  liveEvents,
  liveRunId,
  liveSnapshot,
  onClose,
}: RunExecutionWorkbenchProps) {
  const rootRef = useRef<HTMLElement>(null);
  const workbench = useRunExecutionWorkbench({
    initialSource,
    liveEvents,
    liveRunId,
    liveSnapshot,
  });
  useEffect(() => rootRef.current?.focus(), []);

  return (
    <section
      ref={rootRef}
      tabIndex={-1}
      aria-label="运行执行台"
      className="grid h-full min-h-0 min-w-0 grid-rows-[56px_minmax(0,1fr)_112px] overflow-hidden bg-white outline-none"
      onKeyDown={(event) => {
        if (event.key === 'Escape') onClose();
      }}
    >
      <WorkbenchHeader
        source={workbench.source}
        view={workbench.view}
        liveAvailable={Boolean(liveSnapshot || liveRunId)}
        runs={workbench.history.runs}
        selectedRunId={workbench.history.selectedRunId}
        loading={workbench.history.loading}
        onSourceChange={workbench.setSource}
        onViewChange={workbench.setView}
        onSelectRun={workbench.selectHistoryRun}
        onRefresh={() => void workbench.history.refresh()}
        onClose={onClose}
      />
      <main className="min-h-0 min-w-0 overflow-hidden">
        {workbench.workflow && workbench.presentation ? (
          <WorkbenchStage
            view={workbench.view}
            workflow={workbench.workflow}
            presentation={workbench.presentation}
            playback={workbench.playback}
            details={workbench.details}
          />
        ) : (
          <div className="flex h-full items-center justify-center bg-slate-50 p-8 text-center">
            <div>
              <History className="mx-auto size-9 text-slate-300" aria-hidden="true" />
              <h2 className="mt-3 text-[15px] font-semibold text-slate-700">选择一次运行</h2>
              <p className="mt-1 text-[13px] text-slate-500">运行中的流程或历史记录会在同一执行台中打开。</p>
            </div>
          </div>
        )}
      </main>
      <RunPlaybackTransport
        events={workbench.events}
        cursor={workbench.cursor}
        presentation={workbench.presentation}
        followLatest={workbench.followLatest}
        currentSource={workbench.source === 'current'}
        onCursorChange={workbench.setCursor}
        onReturnToLatest={workbench.returnToLatest}
      />
    </section>
  );
}

type WorkbenchHeaderProps = Readonly<{
  source: RunWorkbenchSource;
  view: RunWorkbenchView;
  liveAvailable: boolean;
  runs: ReadonlyArray<{ run_id: string; workflow_name: string; started_at_unix_ms: number }>;
  selectedRunId: string | null;
  loading: boolean;
  onSourceChange: (source: RunWorkbenchSource) => void;
  onViewChange: (view: RunWorkbenchView) => void;
  onSelectRun: (runId: string) => void;
  onRefresh: () => void;
  onClose: () => void;
}>;

function WorkbenchHeader({
  source,
  view,
  liveAvailable,
  runs,
  selectedRunId,
  loading,
  onSourceChange,
  onViewChange,
  onSelectRun,
  onRefresh,
  onClose,
}: WorkbenchHeaderProps) {
  return (
    <header className="flex min-w-0 items-center gap-4 border-b border-slate-200 bg-white px-4">
      <div className="flex shrink-0 items-center rounded-md bg-slate-100 p-1">
        {(['current', 'history'] as const).map((candidate) => (
          // 数据源切换是执行台业务分段控件，不是通用操作按钮。
          <button
            key={candidate}
            type="button"
            disabled={candidate === 'current' && !liveAvailable}
            className={
              'rounded px-3 py-1.5 text-[12px] font-semibold disabled:cursor-not-allowed disabled:opacity-40 ' +
              (source === candidate ? 'bg-white text-blue-700 shadow-sm' : 'text-slate-500 hover:text-slate-900')
            }
            onClick={() => onSourceChange(candidate)}
          >
            {candidate === 'current' ? '当前运行' : '历史记录'}
          </button>
        ))}
      </div>
      {source === 'history' ? (
        <div className="flex min-w-0 max-w-sm items-center gap-1">
          <Select
            aria-label="历史运行记录"
            density="compact"
            value={selectedRunId ?? ''}
            containerClassName="min-w-0 w-72"
            options={runs.map((run) => ({
              value: run.run_id,
              label: `${run.workflow_name} · ${new Date(run.started_at_unix_ms).toLocaleString()}`,
            }))}
            disabled={runs.length === 0}
            onValueChange={onSelectRun}
          />
          <IconButton
            icon={RefreshCw}
            label="刷新运行记录"
            disabled={loading}
            iconClassName={`size-3.5 ${loading ? 'animate-spin' : ''}`}
            onClick={onRefresh}
          />
        </div>
      ) : null}
      <nav className="ml-auto flex h-full shrink-0 items-center">
        {MAIN_VIEWS.map((item) => (
          // 主舞台页签承担执行台业务导航。
          <button
            key={item.id}
            type="button"
            className={
              'relative h-full px-4 text-[13px] font-semibold ' +
              (view === item.id
                ? 'text-blue-700 after:absolute after:inset-x-3 after:bottom-0 after:h-0.5 after:bg-blue-600'
                : 'text-slate-500 hover:text-slate-900')
            }
            onClick={() => onViewChange(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      <Button variant="ghost" size="compact" icon={X} onClick={onClose}>关闭执行台</Button>
    </header>
  );
}

type WorkbenchStageProps = Readonly<{
  view: RunWorkbenchView;
  workflow: NonNullable<ReturnType<typeof useRunExecutionWorkbench>['workflow']>;
  presentation: NonNullable<ReturnType<typeof useRunExecutionWorkbench>['presentation']>;
  playback: ReturnType<typeof useRunExecutionWorkbench>['playback'];
  details: ReturnType<typeof useRunExecutionWorkbench>['details'];
}>;

function WorkbenchStage({
  view,
  workflow,
  presentation,
  playback,
  details,
}: WorkbenchStageProps) {
  if (view === 'flow') {
    return <RunFlowStage workflow={workflow} presentation={presentation} playback={playback} />;
  }
  if (view === 'scene') {
    return (
      <RunSceneStage
        details={details}
        selectedNodeId={playback.selectedEvent?.expanded_node_id ?? playback.selectedEvent?.node_id ?? null}
        selectedNodeSequence={playback.selectedNodeSequence}
        cursorSequence={playback.selectedEvent?.sequence ?? null}
        sceneInvalidatedAtSequence={playback.sceneInvalidatedAtSequence}
      />
    );
  }
  return (
    <RunDataStage
      details={details}
      selectedEvent={playback.selectedEvent}
      selectedNodeSequence={playback.selectedNodeSequence}
      presentation={presentation}
    />
  );
}
