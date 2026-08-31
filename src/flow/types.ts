import type { ComponentType } from 'react';

/** 画布逻辑坐标。 */
export type FlowPoint = { /** 水平坐标。 */ x: number; /** 垂直坐标。 */ y: number };
/** 轴对齐矩形，尺寸使用画布逻辑像素。 */
export type FlowRect = FlowPoint & { /** 矩形宽度。 */ width: number; /** 矩形高度。 */ height: number };
/** 节点端点可停靠的四条边。 */
export type FlowAnchorSide = 'top' | 'right' | 'bottom' | 'left';
/** 连线保存的节点端点偏好；路由器仍可在自动模式中换边。 */
export type FlowEndpoint = { /** 端点所属节点。 */ nodeId: string; /** 用户选择的首选停靠边。 */ side?: FlowAnchorSide };
/** 当前视口的平移和缩放。 */
export type ViewportTransform = { /** 水平平移。 */ x: number; /** 垂直平移。 */ y: number; /** 缩放倍率。 */ zoom: number };

/** 路由器当前所处的画布交互阶段。 */
export type RoutingInteraction =
  | Readonly<{
      /** 空闲阶段允许把本轮脏边提交给 Worker 精修。 */
      kind: 'idle';
    }>
  | Readonly<{
      /** 节点拖拽阶段只执行主线程增量预览。 */
      kind: 'node-drag';
      /** 本次拖拽涉及的节点 ID；路由索引据此避免扫描全部节点。 */
      nodeIds: ReadonlyArray<string>;
      /** 区分相邻两次拖拽的单调递增标识。 */
      interactionId: number;
    }>;

/** 与具体业务数据解耦的通用 Flow 节点。 */
export type FlowNode<TData = unknown> = {
  /** 文档内唯一节点 ID。 */
  id: string;
  /** 业务注册表中的节点类型。 */
  kind: string;
  /** 节点左上角世界坐标。 */
  position: FlowPoint;
  /** 固定逻辑尺寸，用于选择和路由。 */
  size: { width: number; height: number };
  /** 业务自定义数据。 */
  data: TData;
};

/** 与具体业务边数据解耦的有向连接。 */
export type FlowEdge<TEdgeData = unknown> = {
  /** 文档内唯一连线 ID。 */
  id: string;
  /** 有向连线的起始端。 */
  source: FlowEndpoint;
  /** 有向连线的目标端。 */
  target: FlowEndpoint;
  /** 业务自定义边数据。 */
  data: TEdgeData;
};

/** 业务层交给通用画布渲染的连线文字与颜色。 */
export type FlowEdgeLabel = Readonly<{
  /** 显示在连线旁的简短文字。 */
  text: string;
  /** 与分支语义对应的 SVG 颜色。 */
  color: string;
}>;

/** 将未知业务边数据转换成可选的通用连线标签。 */
export type FlowEdgeLabelResolver = (data: unknown) => FlowEdgeLabel | null;

/** 节点渲染器从 Flow 内核接收的最小状态。 */
export type FlowNodeRendererProps<TData = unknown> = {
  /** 当前节点完整快照。 */
  node: FlowNode<TData>;
  /** 当前节点是否处于选中集合。 */
  selected: boolean;
};

/** 由业务注册节点尺寸、渲染器和连接约束。 */
export type NodeDefinition<TData = unknown> = {
  /** 注册表键和节点 kind。 */
  kind: string;
  /** 面向用户的节点名称。 */
  title: string;
  /** 新建节点的默认尺寸。 */
  defaultSize: { width: number; height: number };
  /** 节点业务渲染器。 */
  component: ComponentType<FlowNodeRendererProps<TData>>;
  /** 是否在同一文档中只允许一个实例。 */
  singleton?: boolean;
  /** 是否允许从该节点创建连线。 */
  canStartConnection?: boolean;
  /** 是否允许该节点作为目标。 */
  canEndConnection?: boolean;
};

/** 按节点 kind 索引的定义注册表。 */
export type NodeRegistry = Record<string, NodeDefinition<any>>;

/** 一条经过路由的折线路径。 */
export type RoutedEdge = {
  /** 对应的业务连线 ID。 */
  edgeId: string;
  /** 简化后的正交折点。 */
  points: ReadonlyArray<FlowPoint>;
  /** 可直接渲染的圆角 SVG path。 */
  path: string;
  /** 路由器最终选择的源侧。 */
  sourceSide: FlowAnchorSide;
  /** 路由器最终选择的目标侧。 */
  targetSide: FlowAnchorSide;
  /** 路径包围盒，用于视口裁剪。 */
  bounds: FlowRect;
};
