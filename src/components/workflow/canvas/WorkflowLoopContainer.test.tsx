import { act, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  createFlowStore,
  FlowProvider,
} from '../../../flow';
import type {
  WorkflowCanvasNode,
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../../features/workflow';
import { WorkflowLoopContainer } from './WorkflowLoopContainer';
import { WorkflowNodeCard } from './WorkflowNodeCard';

describe('WorkflowLoopContainer', () => {
  it('renders the live child node components and their real edge', () => {
    const entry = node('entry', 'loopEntry', 40, 80, '每轮开始');
    const log = node('log', 'log', 240, 80, '记录本轮结果');
    const loop = loopNode();
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      activeDocumentId: 'root',
      nodes: [loop],
      edges: [],
      documents: {
        root: { nodes: [loop], edges: [] },
        body: {
          nodes: [entry, log],
          edges: [{
            id: 'entry-to-log',
            source: { nodeId: entry.id },
            target: { nodeId: log.id },
            data: { branch: null },
          }],
        },
      },
    });
    const { container } = render(
      <FlowProvider store={store}>
        <WorkflowLoopContainer
          node={loop}
          nodeRenderer={WorkflowNodeCard}
          selected={false}
        />
      </FlowProvider>,
    );

    expect(screen.getByText('记录本轮结果')).toBeVisible();
    expect(screen.getByText('初始消息')).toBeVisible();
    expect(container.querySelector('[data-loop-edge-id="entry-to-log"]')).not.toBeNull();
    const loopLabel = container.querySelector('[data-loop-label="loop"]');
    expect(loopLabel).toHaveClass('top-1', 'max-w-[calc(50%_-_18px)]');
    expect(loopLabel).not.toHaveClass('-translate-y-1/2');
    expect(screen.queryByText(/双击进入/)).not.toBeInTheDocument();

    act(() => store.setState((state) => ({
      documents: {
        ...state.documents,
        body: {
          ...state.documents.body!,
          nodes: state.documents.body!.nodes.map((child) => child.id === 'log'
            ? {
                ...child,
                data: child.data.kind === 'log'
                  ? { ...child.data, message: '实时更新后的消息' }
                  : child.data,
              }
            : child),
        },
      },
    })));

    expect(screen.getByText('实时更新后的消息')).toBeVisible();
  });
});

/** 建立用于预览的普通或固定边界节点。 */
function node(
  id: string,
  kind: 'log' | 'loopEntry',
  x: number,
  y: number,
  label: string,
): WorkflowCanvasNode {
  return kind === 'log'
    ? {
        id,
        kind,
        position: { x, y },
        size: { width: 142, height: 52 },
        data: {
          kind,
          label,
          message: '初始消息',
          outputBindings: {},
        },
      }
    : {
        id,
        kind,
        position: { x, y },
        size: { width: 118, height: 52 },
        data: { kind, label, outputBindings: {} },
      };
}

/** 建立拥有 body 子作用域的父级 While。 */
function loopNode(): WorkflowCanvasNode {
  return {
    id: 'loop',
    kind: 'loop',
    position: { x: 0, y: 0 },
    size: { width: 300, height: 180 },
    data: {
      kind: 'loop',
      label: '实时 While',
      bodyScopeId: 'body',
      maxIterations: 10,
      timeoutMs: 30_000,
      intervalMs: 500,
      outputBindings: {},
    },
  };
}
