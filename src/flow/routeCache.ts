import type { RouteQuality } from './routingTypes';
import type { FlowEdge, RoutedEdge } from './types';

/** 一条路径及其缓存身份、质量和障碍物版本。 */
export type CachedRoute = Readonly<{
  /** 缓存所属边 ID。 */
  edgeId: string;
  /** 端点身份、方向与几何组成的稳定键。 */
  fingerprint: string;
  /** 当前最后一条已知可见路线。 */
  route: RoutedEdge;
  /** 当前路线的生成阶段。 */
  quality: RouteQuality;
  /** 生成该路线时的障碍物版本。 */
  obstacleRevision: number;
}>;

/** 路由缓存维护稳定数组位置，脏边更新时无需重新遍历全部边。 */
export class RouteCache {
  /** edgeId 到当前缓存条目。 */
  private readonly entries = new Map<string, CachedRoute>();
  /** edgeId 到渲染数组位置。 */
  private readonly routeIndices = new Map<string, number>();
  /** 对 React 暴露的稳定只读数组；内部只在对应脏边位置替换对象。 */
  private routes: RoutedEdge[] = [];

  /** 边集合变化时同步渲染顺序，并移除已删除缓存。 */
  public syncEdges(edges: ReadonlyArray<FlowEdge>): void {
    const currentIds = new Set(edges.map((edge) => edge.id));
    for (const edgeId of this.entries.keys()) {
      if (!currentIds.has(edgeId)) this.entries.delete(edgeId);
    }
    this.routeIndices.clear();
    this.routes = [];
    for (const edge of edges) {
      const cached = this.entries.get(edge.id);
      if (!cached) continue;
      this.routeIndices.set(edge.id, this.routes.length);
      this.routes.push(cached.route);
    }
  }

  /** 读取一条边的当前缓存条目。 */
  public get(edgeId: string): CachedRoute | undefined {
    return this.entries.get(edgeId);
  }

  /** 插入或替换一条脏边路径。 */
  public set(entry: CachedRoute): void {
    this.entries.set(entry.edgeId, entry);
    const routeIndex = this.routeIndices.get(entry.edgeId);
    if (routeIndex === undefined) {
      this.routeIndices.set(entry.edgeId, this.routes.length);
      this.routes.push(entry.route);
      return;
    }
    this.routes[routeIndex] = entry.route;
  }

  /** 删除指定边缓存及其数组位置。 */
  public delete(edgeId: string): void {
    if (!this.entries.delete(edgeId)) return;
    const routeIndex = this.routeIndices.get(edgeId);
    if (routeIndex === undefined) return;
    this.routes.splice(routeIndex, 1);
    this.routeIndices.delete(edgeId);
    for (let index = routeIndex; index < this.routes.length; index += 1) {
      this.routeIndices.set(this.routes[index].edgeId, index);
    }
  }

  /** 返回当前始终可渲染的路线数组。 */
  public values(): ReadonlyArray<RoutedEdge> {
    return this.routes;
  }
}
