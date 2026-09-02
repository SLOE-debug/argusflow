import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { RunDetails, VisualQueryTrace } from '../../../features/workflow';
import { RunSceneStage } from './RunSceneStage';

describe('RunSceneStage', () => {
  it('keeps the screenshot tab selected while the playback event changes', () => {
    const { rerender } = renderScene(details([trace(10)]), 10);

    fireEvent.click(screen.getByRole('tab', { name: '截图标注' }));
    expect(screen.getByRole('tab', { name: '截图标注' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByText('这一窗口和帧没有保存所选图像，请重新运行后再查看。')).toBeVisible();

    rerender(scene(details([trace(20)]), 20));
    expect(screen.getByRole('tab', { name: '截图标注' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByText('这一窗口和帧没有保存所选图像，请重新运行后再查看。')).toBeVisible();

    rerender(scene(details([trace(20)]), 5));
    expect(screen.getByRole('tab', { name: '截图标注' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('heading', { name: '当前事件没有截图' })).toBeVisible();
  });

  it('keeps a long AQL out of the title bar and shows it on demand', () => {
    const longQuery = `all_of(${Array.from({ length: 20 }, (_, index) => (
      `exists(text(name = "message-${index}"))`
    )).join(', ')})`;
    const { container } = renderScene(details([trace(10, longQuery)]), 10);

    const titleBar = container.querySelector('header');
    expect(titleBar).not.toBeNull();
    expect(titleBar).not.toHaveTextContent(longQuery);
    expect(titleBar).toHaveTextContent('没有候选');
    expect(container.firstChild).toHaveClass('min-w-0', 'max-w-full', 'overflow-hidden');

    fireEvent.click(screen.getByRole('button', { name: '查看查询' }));
    const dialog = screen.getByRole('dialog', { name: '本次查找条件' });
    expect(within(dialog).getByText(longQuery)).toBeVisible();

    fireEvent.click(within(dialog).getByRole('button', { name: '关闭查询' }));
    expect(screen.queryByRole('dialog', { name: '本次查找条件' })).not.toBeInTheDocument();
  });
});

/** 渲染带有最小回放选择上下文的场景舞台。 */
function renderScene(runDetails: RunDetails, cursorSequence: number) {
  return render(scene(runDetails, cursorSequence));
}

/** 构造可在 rerender 中复用的场景舞台元素。 */
function scene(runDetails: RunDetails, cursorSequence: number) {
  return (
    <RunSceneStage
      details={runDetails}
      selectedNodeId={null}
      selectedNodeSequence={null}
      cursorSequence={cursorSequence}
      sceneInvalidatedAtSequence={-1}
    />
  );
}

/** 构造只有一个窗口且没有图像 Artifact 的 v2 场景。 */
function trace(
  nodeSequence: number,
  query = 'exists(text(name = $message))',
): VisualQueryTrace {
  return {
    schema_version: 2,
    run_id: 'run-1',
    node_id: `node-${nodeSequence}`,
    node_sequence: nodeSequence,
    query,
    outcome: 'not_found',
    candidate_nodes: [],
    selected_node: null,
    metrics: { elapsed_us: 10, exact_index_hits: 0, scanned_nodes: 1, spatial_candidates: 0 },
    projection: {
      schema_version: 2,
      windows: [{
        window_handle: '100',
        scene_id: nodeSequence,
        frame_id: nodeSequence,
        z_order: 0,
        foreground: true,
        screen_bounds: { x: 0, y: 0, width: 800, height: 600 },
        frame_bounds: { x: 0, y: 0, width: 800, height: 600 },
      }],
      nodes: [],
    },
  };
}

/** 构造场景组件需要的最小历史运行详情。 */
function details(queryTraces: VisualQueryTrace[]): RunDetails {
  return {
    manifest: {
      schema_version: 1,
      run_id: 'run-1',
      workflow_id: 'workflow-1',
      workflow_name: '测试运行',
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
      name: '测试运行',
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
