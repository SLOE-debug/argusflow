import { useCallback, useEffect, useState } from 'react';

import { getRun, isDesktopRuntime, listRuns, readRunEvents } from '../api/workflowApi';
import type {
  ExecutionEvent,
  RunDetails,
  RunManifest,
  RunTraceEvent,
} from '../model/contracts';

/** Run History 与当前画布运行态完全分离的只读查询状态。 */
export type RunHistoryState = Readonly<{
  runs: ReadonlyArray<RunManifest>;
  selectedRunId: string | null;
  selectedRun: RunDetails | null;
  traceEvents: ReadonlyArray<RunTraceEvent>;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  selectRun: (runId: string) => Promise<void>;
}>;

/** 分页能力加入前，按 Manifest 轻量加载列表并按需读取单次 JSONL。 */
export function useRunHistory(
  liveEvents: ReadonlyArray<ExecutionEvent>,
): RunHistoryState {
  const [runs, setRuns] = useState<RunManifest[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [selectedRun, setSelectedRun] = useState<RunDetails | null>(null);
  const [traceEvents, setTraceEvents] = useState<RunTraceEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectRun = useCallback(async (runId: string) => {
    if (!isDesktopRuntime()) return;
    setSelectedRunId(runId);
    setLoading(true);
    setError(null);
    try {
      const [details, events] = await Promise.all([
        getRun(runId),
        readRunEvents(runId),
      ]);
      setSelectedRun(details);
      setTraceEvents(events);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    if (!isDesktopRuntime()) return;
    setLoading(true);
    setError(null);
    try {
      const nextRuns = await listRuns();
      setRuns(nextRuns);
      const nextSelectedId = selectedRunId ?? nextRuns[0]?.run_id ?? null;
      if (nextSelectedId) {
        await selectRun(nextSelectedId);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setLoading(false);
    }
  }, [selectRun, selectedRunId]);

  useEffect(() => {
    void refresh();
  }, []); // 首次挂载只读取一次；选择变化由用户操作驱动。

  /** 最近一个终态序号只用于触发刷新，不把 live event 写入历史选择状态。 */
  const terminalSequence = liveEvents.reduce<number | undefined>((latest, event) => (
    event.kind === 'workflow_completed' || event.kind === 'workflow_failed'
      ? event.sequence
      : latest
  ), undefined);
  useEffect(() => {
    if (terminalSequence !== undefined) void refresh();
  }, [refresh, terminalSequence]);

  return {
    runs,
    selectedRunId,
    selectedRun,
    traceEvents,
    loading,
    error,
    refresh,
    selectRun,
  };
}
