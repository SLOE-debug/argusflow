import type { RunDetails, VisualQueryTrace } from '../model/contracts';

export type RunSceneSelection = Readonly<{
  trace: VisualQueryTrace | null;
  /** Scene 采集对应的 NodeStarted 事件序号，供界面明确标注时间位置。 */
  capturedAtSequence: number | null;
}>;

/**
 * 只选择回放游标之前、且最后一次 UI 写动作后产生的视觉证据。
 *
 * 这两个边界同时阻止“偷看未来场景”和把界面变化前的旧搜索页冒充当前页面。
 */
export function selectRunSceneTrace(
  details: RunDetails | null,
  cursorSequence: number | null,
  selectedNodeId: string | null,
  selectedNodeSequence: number | null,
  sceneInvalidatedAtSequence: number,
): RunSceneSelection {
  if (!details || cursorSequence === null) {
    return { trace: null, capturedAtSequence: null };
  }
  const eligible = details.query_traces.filter((trace) => (
    trace.node_sequence <= cursorSequence
    && trace.node_sequence >= sceneInvalidatedAtSequence
  ));
  const exact = selectedNodeId && selectedNodeSequence !== null
    ? eligible.filter((trace) => (
        trace.node_id === selectedNodeId && trace.node_sequence === selectedNodeSequence
      )).at(-1) ?? null
    : null;
  const trace = exact ?? eligible.at(-1) ?? null;
  return {
    trace,
    capturedAtSequence: trace?.node_sequence ?? null,
  };
}
