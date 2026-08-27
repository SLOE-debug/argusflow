import { captureDocumentSnapshot } from './flowStoreDocument';
import type {
  FlowDocumentSnapshot,
  FlowState,
  HistoryGroup,
} from './flowStoreTypes';

const HISTORY_LIMIT = 100;
const HISTORY_GROUP_WINDOW_MS = 500;

type HistoryState<TData, TEdgeData> = Pick<
  FlowState<TData, TEdgeData>,
  | 'metadata'
  | 'nodes'
  | 'edges'
  | 'past'
  | 'future'
  | 'historyGroup'
>;

/** 历史跳转后需要恢复的文档和选择状态。 */
export type HistoryNavigationState<TData, TEdgeData> = HistoryState<
  TData,
  TEdgeData
> & Pick<FlowState<TData, TEdgeData>, 'selectedNodeIds' | 'selectedEdgeId'>;

/** 应用文档事务，并根据历史分组决定是否新增撤销快照。 */
export function applyDocumentTransaction<TData, TEdgeData>(
  state: HistoryState<TData, TEdgeData>,
  mutate: (
    snapshot: FlowDocumentSnapshot<TData, TEdgeData>,
  ) => FlowDocumentSnapshot<TData, TEdgeData>,
  historyGroupKey?: string,
): HistoryState<TData, TEdgeData> {
  const currentSnapshot = captureDocumentSnapshot(state);
  const nextSnapshot = mutate(currentSnapshot);
  /** reducer 返回同一组文档引用时，不创建空历史，也不发布 Store 更新。 */
  if (
    nextSnapshot.metadata === currentSnapshot.metadata
    && nextSnapshot.nodes === currentSnapshot.nodes
    && nextSnapshot.edges === currentSnapshot.edges
  ) {
    return state;
  }
  const now = Date.now();
  /** 同一分组在短窗口内只保留首次编辑前的历史快照。 */
  const mergeWithCurrentGroup = Boolean(
    historyGroupKey
    && state.historyGroup?.key === historyGroupKey
    && state.historyGroup.expires > now,
  );
  const past = mergeWithCurrentGroup
    ? state.past
    : [
        ...state.past.slice(-(HISTORY_LIMIT - 1)),
        currentSnapshot,
      ];

  return {
    ...nextSnapshot,
    past,
    future: [],
    historyGroup: createHistoryGroup(historyGroupKey, now),
  };
}

/** 返回撤销后的状态；无可用快照时返回 null。 */
export function undoDocument<TData, TEdgeData>(
  state: HistoryState<TData, TEdgeData>,
): HistoryNavigationState<TData, TEdgeData> | null {
  const previous = state.past.at(-1);
  if (!previous) return null;

  return {
    ...previous,
    past: state.past.slice(0, -1),
    future: [captureDocumentSnapshot(state), ...state.future],
    selectedNodeIds: new Set(),
    selectedEdgeId: null,
    historyGroup: null,
  };
}

/** 返回重做后的状态；无可用快照时返回 null。 */
export function redoDocument<TData, TEdgeData>(
  state: HistoryState<TData, TEdgeData>,
): HistoryNavigationState<TData, TEdgeData> | null {
  const next = state.future[0];
  if (!next) return null;

  return {
    ...next,
    past: [...state.past, captureDocumentSnapshot(state)],
    future: state.future.slice(1),
    selectedNodeIds: new Set(),
    selectedEdgeId: null,
    historyGroup: null,
  };
}

/** 创建新的历史合并窗口；未指定分组时清除窗口。 */
function createHistoryGroup(
  key: string | undefined,
  now: number,
): HistoryGroup | null {
  return key
    ? { key, expires: now + HISTORY_GROUP_WINDOW_MS }
    : null;
}
