import { describe, expect, it } from 'vitest';

import type { RunDetails, VisualQueryTrace } from '../model/contracts';
import { selectRunSceneTrace } from './scenePlayback';

describe('selectRunSceneTrace', () => {
  it('never selects a scene captured after the playback cursor', () => {
    const selection = selectRunSceneTrace(
      details([trace(10, 'search'), trace(80, 'sent-message')]),
      40,
      null,
      null,
      -1,
    );

    expect(selection.trace?.query).toBe('search');
  });

  it('drops an old scene after a visible UI mutation until a new observation is captured', () => {
    const run = details([trace(10, 'search')]);

    expect(selectRunSceneTrace(run, 70, null, null, 60).trace).toBeNull();
  });

  it('selects the message verification scene produced after the send action', () => {
    const selection = selectRunSceneTrace(
      details([trace(10, 'search'), trace(85, 'ArgusFlow 测试消息')]),
      116,
      null,
      null,
      60,
    );

    expect(selection.trace?.query).toBe('ArgusFlow 测试消息');
    expect(selection.capturedAtSequence).toBe(85);
  });
});

function trace(nodeSequence: number, query: string): VisualQueryTrace {
  return {
    schema_version: 2,
    run_id: 'run-1',
    node_id: `node-${nodeSequence}`,
    node_sequence: nodeSequence,
    query,
    outcome: 'unique',
    candidate_nodes: [],
    selected_node: null,
    metrics: { elapsed_us: 1, exact_index_hits: 1, scanned_nodes: 1, spatial_candidates: 1 },
    projection: { schema_version: 2, windows: [], nodes: [] },
  };
}

function details(queryTraces: VisualQueryTrace[]): RunDetails {
  return {
    manifest: {
      schema_version: 1,
      run_id: 'run-1',
      workflow_id: 'workflow-1',
      workflow_name: '测试',
      started_at_unix_ms: 0,
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
      name: '测试',
      inputs: [],
      variables: {},
      permissions: { allow: [] },
      graph: { root_scope_id: 'root', scopes: [] },
    },
    presentation: { schema_version: 1, node_labels: {} },
    nodes: [],
    artifacts: [],
    query_traces: queryTraces,
  };
}
