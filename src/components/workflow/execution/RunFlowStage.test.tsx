import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { RunPlaybackState, WorkflowDefinition } from '../../../features/workflow';
import { RunFlowStage } from './RunFlowStage';

describe('RunFlowStage', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', class {
      public constructor(private readonly callback: ResizeObserverCallback) {}

      public observe(): void {
        this.callback([{
          contentRect: { width: 1_200, height: 700 },
        } as ResizeObserverEntry], this as unknown as ResizeObserver);
      }

      public disconnect(): void {}
      public unobserve(): void {}
    });
    vi.stubGlobal('Worker', class {
      public postMessage(): void {}
      public addEventListener(): void {}
      public removeEventListener(): void {}
      public terminate(): void {}
    });
  });

  it('renders a selected while child inside its parent flow without scope replacement', () => {
    const workflow = nestedWorkflow();
    const playback: RunPlaybackState = {
      cursor: 2,
      selectedEvent: event('child'),
      selectedFlowNodeId: 'child',
      nodeStates: new Map([['loop', 'running'], ['child', 'running']]),
      nodeExecutionCounts: new Map([['loop', 1], ['child', 3]]),
      activeEdgeIds: new Set(['body-edge']),
      selectedNodeSequence: 2,
      sceneInvalidatedAtSequence: -1,
    };
    const { container } = render(
      <div className="relative h-[700px] w-[1200px]">
        <RunFlowStage
          workflow={workflow}
          presentation={{
            schema_version: 1,
            node_labels: { loop: '等待搜索页打开', child: '检查搜索页' },
          }}
          playback={playback}
        />
      </div>,
    );

    expect(container.querySelector('[data-run-loop-scope-id="loop-body"]')).not.toBeNull();
    expect(container.querySelector('[data-run-loop-child-node-id="child"]')).not.toBeNull();
    expect(container.textContent).toContain('等待搜索页打开');
    expect(container.textContent).toContain('检查搜索页');
    expect(container.textContent).toContain('第 3 次');
  });
});

function event(nodeId: string) {
  return {
    run_id: 'run-1',
    workflow_id: 'workflow-1',
    sequence: 2,
    node_id: nodeId,
    edge_id: null,
    kind: 'node_started' as const,
    message: null,
    payload: null,
  };
}

function nestedWorkflow(): WorkflowDefinition {
  const node = (id: string, typeId: string, x: number, y: number) => ({
    id,
    position: { x, y },
    size: { width: 160, height: 64 },
    type_id: typeId,
    version: 1,
    payload: {},
    output_bindings: {},
  });
  return {
    schema_version: 10,
    id: 'workflow-1',
    name: '嵌套回放',
    inputs: [],
    variables: {},
    permissions: { allow: [] },
    graph: {
      root_scope_id: 'root',
      scopes: [
        {
          id: 'root',
          parent: null,
          boundary: { type: 'workflow', entry_node_id: 'loop' },
          nodes: [{
            ...node('loop', 'argus.loop', 100, 100),
            size: { width: 420, height: 220 },
            payload: { body_scope_id: 'loop-body', max_iterations: 16 },
          }],
          edges: [],
        },
        {
          id: 'loop-body',
          parent: { scope_id: 'root', node_id: 'loop' },
          boundary: {
            type: 'loop',
            entry_node_id: 'entry',
            continue_node_id: 'child',
            complete_node_id: 'child',
          },
          nodes: [
            node('entry', 'argus.loop.entry', 40, 80),
            node('child', 'argus.ui', 300, 80),
          ],
          edges: [{ id: 'body-edge', source: 'entry', target: 'child', branch: null }],
        },
      ],
    },
  };
}
