import type {
  FlowAnchorSide,
  FlowEdge,
  FlowNode,
  FlowPoint,
  RoutedEdge,
  RoutingInteraction,
} from './types';

/** 路由生成阶段；快速结果用于交互预览，精确结果由空闲 Worker 结算。 */
export type RouteQuality = 'fast' | 'exact' | 'emergency';

/** 精确寻路无法满足全部避障约束时的稳定失败分类。 */
export type RouteFailureReason =
  | 'blocked_port'
  | 'overlapping_nodes'
  | 'search_budget_exceeded';

/** 主线程与 Worker 共用的始终可渲染路由结果。 */
export type RouteResult =
  | Readonly<{
      /** 精确或快速规划已满足当前约束。 */
      kind: 'routed';
      /** 始终可交给 SVG 渲染的正交路线。 */
      route: RoutedEdge;
      /** 当前结果来自主线程预览还是 Worker 精修。 */
      quality: 'fast' | 'exact';
    }>
  | Readonly<{
      /** 极端布局下仅能提供可见降级路线。 */
      kind: 'degraded';
      /** 放宽主体避障后仍保持端口方向的应急路线。 */
      route: RoutedEdge;
      /** 降级路线固定使用 emergency 质量。 */
      quality: 'emergency';
      /** 触发降级的稳定诊断原因。 */
      reason: RouteFailureReason;
    }>;

/** 已解析锚点和安全出口的强类型端口。 */
export type RoutingPort = Readonly<{
  /** 端口所属节点 ID。 */
  nodeId: string;
  /** 端口所在的节点边。 */
  side: FlowAnchorSide;
  /** 节点真实边界上的锚点。 */
  anchor: FlowPoint;
  /** 沿外法线离开节点后的安全出口。 */
  escape: FlowPoint;
}>;

/** 开发态可采集的单轮增量路由指标。 */
export type RouterStats = Readonly<{
  /** 本轮真正重算的边数量。 */
  dirtyEdgeCount: number;
  /** 单条搜索实际读取的最大局部障碍物数量。 */
  nearbyObstacleCount: number;
  /** 本轮使用旧路修补成功的边数量。 */
  fastRepairHits: number;
  /** 本轮构造的可见图顶点总数。 */
  localGraphVertices: number;
  /** 本轮 A* 展开的方向状态总数。 */
  expandedStates: number;
  /** 本轮主线程路由耗时，单位为毫秒。 */
  routeTimeMs: number;
}>;

/** 增量路由引擎的一次不可变输入快照。 */
export type RouteEngineInput = Readonly<{
  /** 当前不可变节点快照。 */
  nodes: ReadonlyArray<FlowNode>;
  /** 当前不可变边快照。 */
  edges: ReadonlyArray<FlowEdge>;
  /** 决定全量同步或拖拽增量更新的交互阶段。 */
  interaction: RoutingInteraction;
}>;

/** React 编排层消费的当前路径与本轮 Worker 精修集合。 */
export type RouteEngineOutput = Readonly<{
  /** 当前全部可渲染路线。 */
  routes: ReadonlyArray<RoutedEdge>;
  /** 本轮被失效传播命中的边 ID。 */
  dirtyEdgeIds: ReadonlySet<string>;
  /** 尚未提交给空闲 Worker 精修的边 ID。 */
  settleEdgeIds: ReadonlySet<string>;
  /** 本轮增量规划的开发态统计。 */
  stats: RouterStats;
}>;
