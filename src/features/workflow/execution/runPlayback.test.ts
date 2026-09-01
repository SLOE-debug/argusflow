import { describe, expect, it } from 'vitest';

import type { ExecutionEvent, WorkflowDefinition } from '../model/contracts';
import { deriveRunPlayback } from './runPlayback';

describe('deriveRunPlayback', () => {
  it('recomputes cumulative node and edge state at an arbitrary event cursor', () => {
    const events = [
      event(0, 'workflow_started'),
      event(1, 'node_started', 'start'),
      event(2, 'node_succeeded', 'start'),
      { ...event(3, 'edge_traversed'), edge_id: 'edge-1' },
      event(4, 'node_started', 'worker'),
      event(5, 'node_succeeded', 'worker'),
    ];

    const playback = deriveRunPlayback(workflow(), events, 4);

    expect(playback.nodeStates.get('start')).toBe('success');
    expect(playback.nodeStates.get('worker')).toBe('running');
    expect(playback.nodeStates.get('end')).toBe('pending');
    expect(playback.activeEdgeIds).toEqual(new Set(['edge-1']));
    expect(playback.selectedNodeSequence).toBe(4);
  });

  it('counts repeated loop executions and preserves the selected occurrence sequence', () => {
    const events = [
      event(0, 'workflow_started'),
      event(1, 'node_started', 'worker'),
      event(2, 'node_succeeded', 'worker'),
      event(3, 'loop_iteration', 'loop'),
      event(4, 'node_started', 'worker'),
      event(5, 'node_succeeded', 'worker'),
    ];

    const playback = deriveRunPlayback(workflow(), events, 4);

    expect(playback.nodeExecutionCounts.get('worker')).toBe(2);
    expect(playback.nodeStates.get('worker')).toBe('running');
    expect(playback.selectedNodeSequence).toBe(4);
  });

  it('marks untouched nodes skipped at a terminal event', () => {
    const playback = deriveRunPlayback(
      workflow(),
      [event(0, 'workflow_started'), event(1, 'workflow_completed')],
      1,
    );

    expect([...playback.nodeStates.values()]).toEqual(['skipped', 'skipped', 'skipped']);
  });

  it('uses an expanded node only when it exists in the persisted snapshot', () => {
    const expandedInner = {
      ...event(1, 'node_started', 'worker'),
      expanded_node_id: 'inner-node',
    };
    const playback = deriveRunPlayback(workflow(), [expandedInner], 0);

    expect(playback.selectedFlowNodeId).toBe('worker');
    expect(playback.nodeStates.get('worker')).toBe('running');
  });

  it('invalidates prior scene evidence after a UI node changes the visible page', () => {
    const definition = workflow();
    const root = definition.graph.scopes[0];
    if (root) root.nodes[1]!.type_id = 'argus.ui';
    const playback = deriveRunPlayback(definition, [
      event(0, 'node_started', 'worker'),
      event(1, 'node_succeeded', 'worker'),
    ], 1);

    expect(playback.sceneInvalidatedAtSequence).toBe(1);
  });
});

function event(
  sequence: number,
  kind: ExecutionEvent['kind'],
  nodeId: string | null = null,
): ExecutionEvent {
  return {
    run_id: 'run-1',
    workflow_id: 'workflow-1',
    sequence,
    node_id: nodeId,
    edge_id: null,
    kind,
    message: null,
    payload: kind === 'loop_iteration'
      ? { type: 'loop_iteration', iteration: 2, max_iterations: 3 }
      : null,
  };
}

function workflow(): WorkflowDefinition {
  return {
    schema_version: 10,
    id: 'workflow-1',
    name: '回放测试',
    inputs: [],
    variables: {},
    permissions: { allow: [] },
    graph: {
      root_scope_id: 'root',
      scopes: [{
        id: 'root',
        parent: null,
        boundary: { type: 'workflow', entry_node_id: 'start' },
        nodes: ['start', 'worker', 'end'].map((id) => ({
          id,
          position: { x: 0, y: 0 },
          size: { width: 160, height: 64 },
          type_id: 'argus.test',
          version: 1,
          payload: {},
          output_bindings: {},
        })),
        edges: [],
      }],
    },
  };
}
