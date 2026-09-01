import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { getRun, isDesktopRuntime } from '../api/workflowApi';
import type {
  ExecutionEvent,
  RunDetails,
  RunPresentationSnapshot,
  WorkflowDefinition,
} from '../model/contracts';
import { deriveRunPlayback } from './runPlayback';
import { useRunHistory } from './useRunHistory';

export type RunWorkbenchSource = 'current' | 'history';
export type RunWorkbenchView = 'flow' | 'scene' | 'data';

export type LiveRunSnapshot = Readonly<{
  workflow: WorkflowDefinition;
  presentation: RunPresentationSnapshot;
}>;

type UseRunExecutionWorkbenchOptions = Readonly<{
  initialSource: RunWorkbenchSource;
  liveEvents: ReadonlyArray<ExecutionEvent>;
  liveRunId: string | null;
  liveSnapshot: LiveRunSnapshot | null;
}>;

/** 统一实时与历史数据源，并管理回放游标和跟随尾部状态。 */
export function useRunExecutionWorkbench({
  initialSource,
  liveEvents,
  liveRunId,
  liveSnapshot,
}: UseRunExecutionWorkbenchOptions) {
  const history = useRunHistory(liveEvents);
  const [source, setSourceState] = useState<RunWorkbenchSource>(initialSource);
  const [view, setView] = useState<RunWorkbenchView>('flow');
  const [currentDetails, setCurrentDetails] = useState<RunDetails | null>(null);
  const [cursor, setCursorState] = useState(-1);
  const [followLatest, setFollowLatest] = useState(true);
  /** 当前 Run 详情也采用最后请求获胜，防止相邻运行或事件刷新交叉。 */
  const currentDetailsRequest = useRef(0);

  useEffect(() => {
    if (!liveRunId || !isDesktopRuntime()) return;
    const request = ++currentDetailsRequest.current;
    void getRun(liveRunId).then((details) => {
      if (request === currentDetailsRequest.current) setCurrentDetails(details);
    }).catch(() => {
      // 实时 Run 的首个事件可能早于文件系统刷新；后续事件会再次读取。
    });
  }, [liveEvents.length, liveRunId]);

  const events = source === 'current'
    ? liveEvents
    : history.traceEvents.map((trace) => trace.event);
  const details = source === 'current' ? currentDetails : history.selectedRun;
  const workflow = source === 'current'
    ? liveSnapshot?.workflow ?? currentDetails?.workflow ?? null
    : history.selectedRun?.workflow ?? null;
  const presentation = source === 'current'
    ? liveSnapshot?.presentation ?? currentDetails?.presentation ?? null
    : history.selectedRun?.presentation ?? null;

  useEffect(() => {
    if (followLatest) setCursorState(events.length - 1);
  }, [events.length, followLatest]);

  /** 只有当前正在查看的数据源身份变化才重置舞台，后台历史刷新不得打断实时回看。 */
  const activeRunKey = source === 'current' ? liveRunId : history.selectedRunId;
  useEffect(() => {
    setCursorState(events.length - 1);
    setFollowLatest(true);
    setView('flow');
  }, [activeRunKey, source]);

  const setSource = useCallback((nextSource: RunWorkbenchSource) => {
    setSourceState(nextSource);
    setFollowLatest(true);
  }, []);
  const setCursor = useCallback((nextCursor: number) => {
    const bounded = events.length === 0
      ? -1
      : Math.max(0, Math.min(nextCursor, events.length - 1));
    setCursorState(bounded);
    if (source === 'current') setFollowLatest(bounded === events.length - 1);
  }, [events.length, source]);
  const returnToLatest = useCallback(() => {
    setCursorState(events.length - 1);
    setFollowLatest(true);
  }, [events.length]);
  const selectHistoryRun = useCallback((runId: string) => {
    setSourceState('history');
    setFollowLatest(true);
    void history.selectRun(runId);
  }, [history]);
  const playback = useMemo(
    () => deriveRunPlayback(workflow, events, cursor),
    [cursor, events, workflow],
  );

  return {
    source,
    setSource,
    view,
    setView,
    events,
    details,
    workflow,
    presentation,
    playback,
    cursor: playback.cursor,
    setCursor,
    followLatest,
    returnToLatest,
    history,
    selectHistoryRun,
  } as const;
}
