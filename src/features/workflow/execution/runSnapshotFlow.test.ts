import { describe, expect, it } from 'vitest';

import type { WorkflowDefinition } from '../model/contracts';
import {
  createRunSnapshotDocuments,
  findRunNodeDisplayBounds,
} from './runSnapshotFlow';

describe('run snapshot flow', () => {
  it('keeps a while body as an inline child document of the root container', () => {
    const documents = createRunSnapshotDocuments(workflow(), {
      schema_version: 1,
      node_labels: { loop: '等待搜索页打开', child: '检查搜索页' },
    });
    const loop = documents.root?.nodes.find((node) => node.id === 'loop');

    expect(loop?.data.structure).toEqual({
      type: 'loop',
      bodyScopeId: 'loop-body',
      maxIterations: 16,
    });
    expect(documents['loop-body']?.nodes.map((node) => node.id)).toEqual(['entry', 'child']);
  });

  it('projects a nested child into root canvas coordinates for viewport following', () => {
    const bounds = findRunNodeDisplayBounds(workflow(), 'child');

    expect(bounds?.x).toBeCloseTo(282.8);
    expect(bounds?.y).toBeCloseTo(233.6);
    expect(bounds?.width).toBeCloseTo(108.8);
    expect(bounds?.height).toBeCloseTo(43.52);
  });
});

function workflow(): WorkflowDefinition {
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
          boundary: { type: 'workflow', entry_node_id: 'start' },
          nodes: [
            node('start', 'argus.start', 0, 200),
            {
              ...node('loop', 'argus.loop', 100, 200),
              size: { width: 420, height: 220 },
              payload: {
                body_scope_id: 'loop-body',
                max_iterations: 16,
                timeout_ms: 30_000,
                interval_ms: 500,
              },
            },
          ],
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
            node('entry', 'argus.loop.entry', 40, 60),
            node('child', 'argus.ui', 300, 80),
          ],
          edges: [],
        },
      ],
    },
  };
}

function node(id: string, typeId: string, x: number, y: number) {
  return {
    id,
    position: { x, y },
    size: { width: 160, height: 64 },
    type_id: typeId,
    version: 1,
    payload: {},
    output_bindings: {},
  };
}
