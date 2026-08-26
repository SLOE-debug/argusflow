import { ObstacleIndex, type ObstacleChange } from './obstacleIndex';
import { edgeRouteFingerprint } from './routeFingerprint';
import {
  collectDirtyEdgeIds,
  createEdgeAdjacency,
  type EdgeAdjacency,
} from './routeInvalidation';
import { RouteCache } from './routeCache';
import { planEdgeRoute } from './routePlanner';
import { RouteSegmentIndex } from './routeSegmentIndex';
import type {
  RouteEngineInput,
  RouteEngineOutput,
  RouterStats,
} from './routingTypes';
import type { FlowEdge, FlowNode } from './types';
import type { ExactEdgeRouteResponse } from './routingWorkerProtocol';

/** 有缓存、有失效传播与长期空间索引的交互式增量路由引擎。 */
export class FlowRouteEngine {
  /** 节点障碍物只在变化节点上增量写入。 */
  private readonly obstacles = new ObstacleIndex();
  /** 已路由线段用于发现被移动障碍扫到的非邻接边。 */
  private readonly routeSegments = new RouteSegmentIndex();
  /** 始终保留最后一条可见路径及其身份。 */
  private readonly cache = new RouteCache();
  /** 等待空闲 Worker 精修的脏边集合。 */
  private readonly settleEdgeIds = new Set<string>();
  /** 当前节点 ID 到快照数组位置，拖拽帧可 O(moved) 读取新节点。 */
  private readonly nodeIndices = new Map<string, number>();
  /** 当前节点 ID 到最后同步节点。 */
  private readonly nodesById = new Map<string, FlowNode>();
  /** 当前边 ID 到文档边。 */
  private readonly edgesById = new Map<string, FlowEdge>();
  /** 当前节点邻接边索引。 */
  private adjacency: EdgeAdjacency = new Map();
  /** 当前边数组引用；拖拽时不变则跳过全边同步。 */
  private edgeSnapshot: ReadonlyArray<FlowEdge> | null = null;
  /** 当前文档边 ID，用于移除已删除的线段。 */
  private edgeIds = new Set<string>();
  /** 障碍物几何每发生一批变化递增一次。 */
  private obstacleRevision = 0;
  /** 首次 update 前需要完整建立节点与边索引。 */
  private initialized = false;

  /** 按当前交互阶段增量更新缓存，并返回稳定渲染路径。 */
  public update(input: RouteEngineInput): RouteEngineOutput {
    const startedAt = performanceNow();
    const edgeCollectionChanged = this.edgeSnapshot !== input.edges;
    const obstacleChanges = this.syncObstacles(input);
    if (obstacleChanges.length > 0) this.obstacleRevision += 1;

    /** 当前帧只从邻接关系和 swept route query 收集局部脏边。 */
    const dirtyEdgeIds = new Set(collectDirtyEdgeIds(
      obstacleChanges,
      this.adjacency,
      this.routeSegments,
    ));
    if (edgeCollectionChanged) {
      this.syncEdges(input.edges, dirtyEdgeIds);
      /** 邻接表刚重建后，需要用本帧障碍变化再次补齐新邻接边。 */
      for (const edgeId of collectDirtyEdgeIds(
        obstacleChanges,
        this.adjacency,
        this.routeSegments,
      )) dirtyEdgeIds.add(edgeId);
    }

    let nearbyObstacleCount = 0;
    let fastRepairHits = 0;
    let localGraphVertices = 0;
    let expandedStates = 0;
    for (const edgeId of dirtyEdgeIds) {
      const edge = this.edgesById.get(edgeId);
      if (!edge) continue;
      const source = this.nodesById.get(edge.source.nodeId);
      const target = this.nodesById.get(edge.target.nodeId);
      if (!source || !target) {
        this.cache.delete(edgeId);
        this.routeSegments.deleteRoute(edgeId);
        continue;
      }
      const previous = this.cache.get(edgeId);
      const planned = planEdgeRoute(
        edge,
        this.nodesById,
        this.obstacles,
        previous?.route,
        'fast',
      );
      if (!planned) continue;
      const fingerprint = edgeRouteFingerprint(edge, source, target);
      this.cache.set({
        edgeId,
        fingerprint,
        route: planned.result.route,
        quality: planned.result.quality,
        obstacleRevision: this.obstacleRevision,
      });
      this.routeSegments.setRoute(planned.result.route);
      this.settleEdgeIds.add(edgeId);
      nearbyObstacleCount = Math.max(
        nearbyObstacleCount,
        planned.stats.nearbyObstacleCount,
      );
      fastRepairHits += planned.stats.fastRepairHits;
      localGraphVertices += planned.stats.localGraphVertices;
      expandedStates += planned.stats.expandedStates;
    }
    /** 新增边完成首轮规划后按文档顺序整理渲染数组。 */
    if (edgeCollectionChanged) this.cache.syncEdges(input.edges);
    this.initialized = true;

    const stats: RouterStats = {
      dirtyEdgeCount: dirtyEdgeIds.size,
      nearbyObstacleCount,
      fastRepairHits,
      localGraphVertices,
      expandedStates,
      routeTimeMs: performanceNow() - startedAt,
    };
    return {
      routes: this.cache.values(),
      dirtyEdgeIds,
      settleEdgeIds: new Set(this.settleEdgeIds),
      stats,
    };
  }

  /** 取出当前待精修集合；只有 idle 编排层应调用。 */
  public takeSettleEdgeIds(): ReadonlySet<string> {
    const edgeIds = new Set(this.settleEdgeIds);
    this.settleEdgeIds.clear();
    return edgeIds;
  }

  /**
   * 应用 Worker 返回的精确 patch。
   *
   * failed 结果及已过期 fingerprint 都不会覆盖 Last Known Good。
   */
  public applyExactRoutes(
    responses: ReadonlyArray<ExactEdgeRouteResponse>,
  ): boolean {
    let changed = false;
    for (const response of responses) {
      if (response.kind === 'failed') continue;
      const cached = this.cache.get(response.edgeId);
      if (!cached || cached.fingerprint !== response.fingerprint) continue;
      this.cache.set({
        ...cached,
        route: response.route,
        quality: 'exact',
      });
      this.routeSegments.setRoute(response.route);
      changed = true;
    }
    return changed;
  }

  /** 根据交互阶段选择全量同步或只更新拖拽节点。 */
  private syncObstacles(input: RouteEngineInput): ReadonlyArray<ObstacleChange> {
    if (!this.initialized || input.interaction.kind === 'idle') {
      this.rebuildNodeLookup(input.nodes);
      return this.obstacles.syncAll(input.nodes);
    }
    const changes: ObstacleChange[] = [];
    for (const nodeId of input.interaction.nodeIds) {
      const knownIndex = this.nodeIndices.get(nodeId);
      let node = knownIndex === undefined ? undefined : input.nodes[knownIndex];
      /** 文档结构在拖拽中意外变化时仅为该节点执行一次防御性定位。 */
      if (!node || node.id !== nodeId) {
        const currentIndex = input.nodes.findIndex((candidate) => (
          candidate.id === nodeId
        ));
        if (currentIndex >= 0) {
          this.nodeIndices.set(nodeId, currentIndex);
          node = input.nodes[currentIndex];
        }
      }
      if (!node) continue;
      this.nodesById.set(nodeId, node);
      const change = this.obstacles.updateNode(node);
      if (change) changes.push(change);
    }
    return changes;
  }

  /** 完整同步节点索引只发生在初始化或 idle 快照。 */
  private rebuildNodeLookup(nodes: ReadonlyArray<FlowNode>): void {
    this.nodeIndices.clear();
    this.nodesById.clear();
    nodes.forEach((node, index) => {
      this.nodeIndices.set(node.id, index);
      this.nodesById.set(node.id, node);
    });
  }

  /** 边文档变化时重建邻接与 ID 索引，并按 fingerprint 标脏。 */
  private syncEdges(
    edges: ReadonlyArray<FlowEdge>,
    dirtyEdgeIds: Set<string>,
  ): void {
    const nextEdgeIds = new Set(edges.map((edge) => edge.id));
    for (const edgeId of this.edgeIds) {
      if (nextEdgeIds.has(edgeId)) continue;
      this.cache.delete(edgeId);
      this.routeSegments.deleteRoute(edgeId);
      this.settleEdgeIds.delete(edgeId);
    }
    this.cache.syncEdges(edges);
    this.edgesById.clear();
    for (const edge of edges) {
      this.edgesById.set(edge.id, edge);
      const source = this.nodesById.get(edge.source.nodeId);
      const target = this.nodesById.get(edge.target.nodeId);
      const cached = this.cache.get(edge.id);
      if (
        !source
        || !target
        || !cached
        || cached.fingerprint !== edgeRouteFingerprint(edge, source, target)
      ) dirtyEdgeIds.add(edge.id);
    }
    this.adjacency = createEdgeAdjacency(edges);
    this.edgeSnapshot = edges;
    this.edgeIds = nextEdgeIds;
  }
}

/** performance 不可用的非浏览器测试环境回退到 Date。 */
function performanceNow(): number {
  return typeof performance === 'undefined' ? Date.now() : performance.now();
}
