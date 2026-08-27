import { rectsIntersect } from '../geometry/geometry';
import { segmentBounds } from './routingGeometry';
import { SpatialIndexById } from './spatialIndexById';
import type { FlowPoint, FlowRect, RoutedEdge } from '../types';

/** 路由空间索引中的稳定线段引用。 */
export type RouteSegmentRef = Readonly<{
  /** 线段所属边 ID。 */
  edgeId: string;
  /** 线段在路线折点数组中的顺序。 */
  segmentIndex: number;
  /** 线段起点。 */
  start: FlowPoint;
  /** 线段终点。 */
  end: FlowPoint;
  /** 线段用于空间查询的精确包围盒。 */
  bounds: FlowRect;
}>;

/** 按 edgeId 增量替换线段的长期空间索引。 */
export class RouteSegmentIndex {
  /** 稳定 segment key 对应的空间条目。 */
  private readonly index = new SpatialIndexById<string, RouteSegmentRef>();
  /** 每条边当前拥有的 segment key，用于局部删除。 */
  private readonly keysByEdge = new Map<string, ReadonlyArray<string>>();

  /** 用新路线替换指定边的全部旧线段。 */
  public setRoute(route: RoutedEdge): void {
    this.deleteRoute(route.edgeId);
    const keys: string[] = [];
    for (let segmentIndex = 1; segmentIndex < route.points.length; segmentIndex += 1) {
      const start = route.points[segmentIndex - 1];
      const end = route.points[segmentIndex];
      const bounds = segmentBounds(start, end);
      const key = `${route.edgeId}:${segmentIndex - 1}`;
      this.index.set(key, {
        edgeId: route.edgeId,
        segmentIndex: segmentIndex - 1,
        start,
        end,
        bounds,
      }, bounds);
      keys.push(key);
    }
    this.keysByEdge.set(route.edgeId, keys);
  }

  /** 删除一条边的全部线段。 */
  public deleteRoute(edgeId: string): void {
    const keys = this.keysByEdge.get(edgeId);
    if (!keys) return;
    for (const key of keys) this.index.delete(key);
    this.keysByEdge.delete(edgeId);
  }

  /** 查询与 swept 区域真正相交的路由边 ID。 */
  public queryEdgeIds(bounds: FlowRect): ReadonlySet<string> {
    const edgeIds = new Set<string>();
    for (const segment of this.index.query(bounds)) {
      if (rectsIntersect(segment.bounds, bounds)) edgeIds.add(segment.edgeId);
    }
    return edgeIds;
  }
}
