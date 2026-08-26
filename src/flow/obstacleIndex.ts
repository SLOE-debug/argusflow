import { rectsIntersect } from './geometry';
import { rectEquals } from './routingGeometry';
import { SpatialIndexById } from './spatialIndexById';
import type { FlowNode, FlowRect } from './types';

/** 普通节点周围预留的正交路由安全距离。 */
export const ROUTING_OBSTACLE_GAP = 16;

/** 障碍物索引保存节点与真实、膨胀边界，供端口和主体分别使用。 */
export type IndexedObstacle = Readonly<{
  /** 对应节点的稳定 ID。 */
  nodeId: string;
  /** 包含路由安全距离的禁止矩形。 */
  rect: FlowRect;
}>;

/** 单个节点障碍物的增量变化。 */
export type ObstacleChange = Readonly<{
  /** 发生几何变化的节点 ID。 */
  nodeId: string;
  /** 更新前的安全区；新增节点时为 null。 */
  previousRect: FlowRect | null;
  /** 更新后的安全区；删除节点时为 null。 */
  currentRect: FlowRect | null;
}>;

/** 长期复用、按节点 ID 增量维护的障碍物空间索引。 */
export class ObstacleIndex {
  /** 节点 ID 到空间条目的稳定索引。 */
  private readonly index = new SpatialIndexById<string, IndexedObstacle>();

  /** 读取指定节点对应的当前障碍物。 */
  public get(nodeId: string): IndexedObstacle | undefined {
    return this.index.get(nodeId);
  }

  /** 增量写入一个节点；几何未变化时不产生失效记录。 */
  public updateNode(node: FlowNode): ObstacleChange | null {
    const previous = this.index.get(node.id);
    const current = indexedObstacle(node);
    if (previous && rectEquals(previous.rect, current.rect)) return null;
    this.index.set(node.id, current, current.rect);
    return {
      nodeId: node.id,
      previousRect: previous?.rect ?? null,
      currentRect: current.rect,
    };
  }

  /** 删除一个节点并返回其旧安全区，用于 swept invalidation。 */
  public deleteNode(nodeId: string): ObstacleChange | null {
    const previous = this.index.get(nodeId);
    if (!previous) return null;
    this.index.delete(nodeId);
    return {
      nodeId,
      previousRect: previous.rect,
      currentRect: null,
    };
  }

  /**
   * 对非拖拽文档快照执行完整同步。
   *
   * 拖拽热路径由路由引擎根据 interaction.nodeIds 调用 updateNode，不进入此扫描。
   */
  public syncAll(nodes: ReadonlyArray<FlowNode>): ReadonlyArray<ObstacleChange> {
    const changes: ObstacleChange[] = [];
    const currentIds = new Set(nodes.map((node) => node.id));
    for (const nodeId of this.index.ids()) {
      if (currentIds.has(nodeId)) continue;
      const change = this.deleteNode(nodeId);
      if (change) changes.push(change);
    }
    for (const node of nodes) {
      const change = this.updateNode(node);
      if (change) changes.push(change);
    }
    return changes;
  }

  /** 查询区域内真正发生矩形相交的障碍物，过滤空间桶假阳性。 */
  public query(bounds: FlowRect): ReadonlyArray<IndexedObstacle> {
    return [...this.index.query(bounds)].filter((obstacle) => (
      rectsIntersect(obstacle.rect, bounds)
    ));
  }

  /** 返回全部障碍物快照，供全局 OVG 最后一级扩张使用。 */
  public all(): ReadonlyArray<IndexedObstacle> {
    return [...this.index.values()];
  }
}

/** 将节点转换为带路由安全距离的障碍物条目。 */
export function indexedObstacle(node: FlowNode): IndexedObstacle {
  const actualRect = { ...node.position, ...node.size };
  return {
    nodeId: node.id,
    rect: {
      x: actualRect.x - ROUTING_OBSTACLE_GAP,
      y: actualRect.y - ROUTING_OBSTACLE_GAP,
      width: actualRect.width + ROUTING_OBSTACLE_GAP * 2,
      height: actualRect.height + ROUTING_OBSTACLE_GAP * 2,
    },
  };
}
