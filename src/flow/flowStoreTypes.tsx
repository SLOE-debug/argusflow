import type { AlignMode, DistributeMode } from './selection';
import type {
  FlowAnchorSide,
  FlowEdge,
  FlowNode,
  FlowPoint,
  ViewportTransform,
} from './types';

/** 可进入历史记录的 Flow 文档快照。 */
export type FlowDocumentSnapshot<TData, TEdgeData> = Readonly<{
  /** 由业务层保存的工作流名称、变量等文档字段。 */
  metadata: Readonly<Record<string, unknown>>;
  /** 快照中的节点集合。 */
  nodes: ReadonlyArray<FlowNode<TData>>;
  /** 快照中的连线集合。 */
  edges: ReadonlyArray<FlowEdge<TEdgeData>>;
}>;

/** Flow 内部剪贴板保存的完整选中子图。 */
export type FlowClipboard<TData, TEdgeData> = Readonly<Pick<
  FlowDocumentSnapshot<TData, TEdgeData>,
  'nodes' | 'edges'
>>;

/** 框选覆盖层的世界坐标。 */
export type SelectionBox = Readonly<{
  start: FlowPoint;
  end: FlowPoint;
}>;

/** 新建或重连时显示的临时连线。 */
export type ConnectionDraft = Readonly<{
  nodeId: string;
  side: FlowAnchorSide;
  point: FlowPoint;
  edgeId?: string;
  endpoint?: 'source' | 'target';
}>;

/** 文本连续编辑所使用的历史合并窗口。 */
export type HistoryGroup = Readonly<{
  key: string;
  expires: number;
}>;

/** 通用 Flow 文档、视口、选择、交互、历史和运行状态。 */
export type FlowState<TData = unknown, TEdgeData = unknown> = {
  /** 随节点和边一起进入撤销历史的业务文档字段。 */
  metadata: Readonly<Record<string, unknown>>;
  /** 当前文档节点。 */
  nodes: ReadonlyArray<FlowNode<TData>>;
  /** 当前文档连线。 */
  edges: ReadonlyArray<FlowEdge<TEdgeData>>;
  /** 当前画布平移和缩放。 */
  viewport: ViewportTransform;
  /** 当前选中的节点 ID。 */
  selectedNodeIds: Set<string>;
  /** 当前选中的单条连线 ID。 */
  selectedEdgeId: string | null;
  /** 当前悬停节点 ID。 */
  hoveredNodeId: string | null;
  /** 当前悬停连线 ID。 */
  hoveredEdgeId: string | null;
  /** 当前框选手势；不存在手势时为 null。 */
  selectionBox: SelectionBox | null;
  /** 当前连线手势；不存在手势时为 null。 */
  connectionDraft: ConnectionDraft | null;
  /** 运行态连线 ID 到过期时间戳的映射。 */
  activeEdgeIds: Record<string, number>;
  /** 可撤销的文档快照。 */
  past: ReadonlyArray<FlowDocumentSnapshot<TData, TEdgeData>>;
  /** 可重做的文档快照。 */
  future: ReadonlyArray<FlowDocumentSnapshot<TData, TEdgeData>>;
  /** 最近一次复制或粘贴形成的子图。 */
  clipboard: FlowClipboard<TData, TEdgeData> | null;
  /** 文本连续编辑合并使用的历史分组及到期时间。 */
  historyGroup: HistoryGroup | null;
  /** 替换当前视口。 */
  setViewport: (viewport: ViewportTransform) => void;
  /** 替换节点集合，并可选择是否记录历史。 */
  setNodes: (nodes: ReadonlyArray<FlowNode<TData>>, record?: boolean) => void;
  /** 替换连线集合，并可选择是否记录历史。 */
  setEdges: (edges: ReadonlyArray<FlowEdge<TEdgeData>>, record?: boolean) => void;
  /** 合并业务文档字段。 */
  setMetadata: (
    metadata: Record<string, unknown>,
    record?: boolean,
    historyGroup?: string,
  ) => void;
  /** 执行文档事务；同一 historyGroup 在 500ms 内合并为一次撤销。 */
  transact: (
    mutate: (
      snapshot: FlowDocumentSnapshot<TData, TEdgeData>,
    ) => FlowDocumentSnapshot<TData, TEdgeData>,
    historyGroup?: string,
  ) => void;
  /** 按指定模式更新节点选择。 */
  selectNodes: (
    ids: Iterable<string>,
    mode?: 'replace' | 'add' | 'toggle',
  ) => void;
  /** 选择单条连线，并清除节点选择。 */
  selectEdge: (id: string | null) => void;
  /** 清除节点和连线选择。 */
  clearSelection: () => void;
  /** 更新悬停节点。 */
  setHoveredNode: (id: string | null) => void;
  /** 更新悬停连线。 */
  setHoveredEdge: (id: string | null) => void;
  /** 更新框选手势。 */
  setSelectionBox: (box: SelectionBox | null) => void;
  /** 更新连线手势。 */
  setConnectionDraft: (draft: ConnectionDraft | null) => void;
  /** 移动所有选中节点，并可选择是否记录历史。 */
  moveSelected: (delta: FlowPoint, record?: boolean) => void;
  /** 对齐选中节点。 */
  align: (mode: AlignMode) => void;
  /** 均匀分布选中节点。 */
  distribute: (mode: DistributeMode) => void;
  /** 删除选择，同时保留受保护种类的节点。 */
  deleteSelection: (protectedKinds?: Set<string>) => void;
  /** 复制选中子图。 */
  copy: () => void;
  /** 粘贴剪贴板子图，同时跳过冲突的单例节点。 */
  paste: (singletonKinds?: Set<string>) => void;
  /** 复制并立即粘贴选中子图。 */
  duplicate: (singletonKinds?: Set<string>) => void;
  /** 撤销最近一次文档事务。 */
  undo: () => void;
  /** 重做最近一次撤销。 */
  redo: () => void;
  /** 临时激活一条连线以展示运行粒子。 */
  activateEdge: (edgeId: string, duration?: number) => void;
};
