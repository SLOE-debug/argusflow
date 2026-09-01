import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { RunDetails, RunTraceEvent } from '../model/contracts';
import { useRunHistory } from './useRunHistory';

const api = vi.hoisted(() => ({
  getRun: vi.fn(),
  readRunEvents: vi.fn(),
  listRuns: vi.fn(),
  isDesktopRuntime: vi.fn(() => true),
}));

vi.mock('../api/workflowApi', () => api);

describe('useRunHistory', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listRuns.mockResolvedValue([]);
  });

  it('keeps the newest history selection when earlier requests finish later', async () => {
    const runA = deferred<RunDetails>();
    const eventsA = deferred<RunTraceEvent[]>();
    const runB = deferred<RunDetails>();
    const eventsB = deferred<RunTraceEvent[]>();
    api.getRun.mockImplementation((runId: string) => runId === 'a' ? runA.promise : runB.promise);
    api.readRunEvents.mockImplementation((runId: string) => (
      runId === 'a' ? eventsA.promise : eventsB.promise
    ));
    const { result } = renderHook(() => useRunHistory([]));
    await waitFor(() => expect(api.listRuns).toHaveBeenCalled());

    act(() => {
      void result.current.selectRun('a');
      void result.current.selectRun('b');
    });
    await act(async () => {
      runB.resolve(runDetails('b'));
      eventsB.resolve([]);
    });
    await waitFor(() => expect(result.current.selectedRun?.manifest.run_id).toBe('b'));
    await act(async () => {
      runA.resolve(runDetails('a'));
      eventsA.resolve([]);
    });

    expect(result.current.selectedRunId).toBe('b');
    expect(result.current.selectedRun?.manifest.run_id).toBe('b');
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => { resolve = resolver; });
  return { promise, resolve };
}

function runDetails(runId: string): RunDetails {
  return {
    manifest: {
      schema_version: 1,
      run_id: runId,
      workflow_id: 'workflow-1',
      workflow_name: runId,
      started_at_unix_ms: 1,
      finished_at_unix_ms: null,
      status: 'running',
      trace_level: 'diagnostics',
      event_count: 0,
      trace_degraded: false,
      failed_node_id: null,
      failure_message: null,
    },
    workflow: {
      schema_version: 10,
      id: 'workflow-1',
      name: runId,
      inputs: [],
      variables: {},
      permissions: { allow: [] },
      graph: { root_scope_id: 'root', scopes: [] },
    },
    presentation: { schema_version: 1, node_labels: {} },
    nodes: [],
    artifacts: [],
    query_traces: [],
  };
}
