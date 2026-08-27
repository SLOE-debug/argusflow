import type { FlowEdge, FlowNode, RoutedEdge } from '../types';
import type { RouteFailureReason } from './routingTypes';

/** 空闲阶段提交给 Worker 的脏边精确结算请求。 */
export type ExactRouteRequest = Readonly<{
  /** 与提交时文档快照绑定的单调版本。 */
  revision: number;
  /** Worker 构建一次障碍物索引所需的节点快照。 */
  nodes: ReadonlyArray<FlowNode>;
  /** Worker 从中筛选脏边的边快照。 */
  edges: ReadonlyArray<FlowEdge>;
  /** 本次需要精修的边 ID。 */
  dirtyEdgeIds: ReadonlyArray<string>;
  /** 精修仅用这些旧路径保持自动端口和局部走廊稳定。 */
  previousRoutes: ReadonlyArray<RoutedEdge>;
}>;

/** 单条边的精确 patch；失败结果禁止删除或覆盖当前可见路线。 */
export type ExactEdgeRouteResponse =
  | Readonly<{
      /** patch 对应边 ID。 */
      edgeId: string;
      /** 已获得满足约束的精确路线。 */
      kind: 'routed';
      /** Worker 启动时计算的端点指纹。 */
      fingerprint: string;
      /** 可合并进当前缓存的精确路线。 */
      route: RoutedEdge;
    }>
  | Readonly<{
      /** patch 对应边 ID。 */
      edgeId: string;
      /** 本轮精确规划失败，主线程必须保留已有路线。 */
      kind: 'failed';
      /** Worker 启动时计算的端点指纹。 */
      fingerprint: string;
      /** 精确规划失败的稳定原因。 */
      reason: RouteFailureReason;
    }>;

/** Worker 一次脏边批处理的版本化响应。 */
export type ExactRouteResponse = Readonly<{
  /** 原样返回请求版本，供主线程丢弃过期结果。 */
  revision: number;
  /** 仅包含请求脏边的可增量合并结果。 */
  routes: ReadonlyArray<ExactEdgeRouteResponse>;
}>;
