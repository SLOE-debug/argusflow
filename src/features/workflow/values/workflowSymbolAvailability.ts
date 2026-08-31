import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
} from '../model/workflowModel';

/** 节点输出相对于某个消费节点的编辑器可用性。 */
export type WorkflowSymbolAvailability = Readonly<{
  /** 输出是否保证在消费节点执行前已经产生。 */
  available: boolean;
  /** 输出不可用时面向值选择器展示的原因。 */
  unavailableReason?: string;
}>;

/** 计算节点输出可用性所需的当前画布快照。 */
export type WorkflowNodeOutputAvailabilityArgs = Readonly<{
  /** 被引用输出所属的生产节点 ID。 */
  producerNodeId: string;
  /** 读取输出的消费节点 ID；省略时不施加控制流约束。 */
  consumerNodeId?: string;
  /** 当前工作流画布节点。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 当前工作流画布连线。 */
  edges: ReadonlyArray<WorkflowCanvasEdge>;
}>;

/** 一次性派生全部生产节点可用性所需的消费节点上下文。 */
export type BuildWorkflowNodeOutputAvailabilityIndexArgs = Readonly<{
  /** 当前读取值的消费节点；省略时工作流数据面板可列出全部输出。 */
  consumerNodeId?: string;
  /** 当前工作流画布节点。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 当前工作流画布连线。 */
  edges: ReadonlyArray<WorkflowCanvasEdge>;
}>;

/** 没有消费节点上下文时，节点输出只需要经过 Registry 枚举即可进入值空间。 */
const AVAILABLE_SYMBOL: WorkflowSymbolAvailability = { available: true };

/**
 * 判断一个节点输出在指定消费节点处是否可安全选择。
 *
 * 有唯一 Start 时沿用 Runtime 的 CFG 语义：生产节点必须从 Start 可达，并且严格支配
 * 消费节点。画布尚未包含 Start 时，则把所有无入边节点视为临时入口，保持编辑中间态可用。
 */
export function getWorkflowNodeOutputAvailability(
  args: WorkflowNodeOutputAvailabilityArgs,
): WorkflowSymbolAvailability {
  return buildWorkflowNodeOutputAvailabilityIndex(args).get(args.producerNodeId)
    ?? unavailable('生产节点不存在');
}

/**
 * 对一个消费节点只构建一次图索引和 dominator 集合。
 *
 * Symbol Registry 会为同一节点列出完整结果和多个 Published Output；如果逐项查询，
 * 会把同一张图重复计算数十次。此索引让所有同源值共享一次可用性结论。
 */
export function buildWorkflowNodeOutputAvailabilityIndex(
  args: BuildWorkflowNodeOutputAvailabilityIndexArgs,
): ReadonlyMap<string, WorkflowSymbolAvailability> {
  if (args.consumerNodeId === undefined) {
    return new Map(args.nodes.map((node) => [node.id, AVAILABLE_SYMBOL]));
  }
  /** 在数组回调外固定已收窄的消费节点，避免可选参数窄化跨闭包丢失。 */
  const consumerNodeId = args.consumerNodeId;

  const nodeIds = new Set(args.nodes.map((node) => node.id));
  if (!nodeIds.has(consumerNodeId)) {
    return new Map(args.nodes.map((node) => [node.id, unavailable('消费节点不存在')]));
  }

  const graph = buildGraph(args.nodes, args.edges);
  if (!graph.reachable.has(consumerNodeId)) {
    return new Map(args.nodes.map((node) => [
      node.id,
      unavailable('消费节点无法从 Start 到达'),
    ]));
  }

  const dominators = computeDominators(
    graph.reachable,
    graph.predecessors,
    graph.entryIds,
  );
  const consumerDominators = dominators.get(consumerNodeId) ?? new Set<string>();
  return new Map(args.nodes.map((node): [string, WorkflowSymbolAvailability] => {
    if (node.id === consumerNodeId) {
      return [node.id, unavailable('生产节点与消费节点相同，当前节点执行前尚未产生该输出')];
    }
    if (!graph.reachable.has(node.id)) {
      return [node.id, unavailable('生产节点无法从 Start 到达')];
    }
    return [
      node.id,
      consumerDominators.has(node.id)
        ? AVAILABLE_SYMBOL
        : unavailable('并非在所有执行路径上可用'),
    ];
  }));
}

/** 以布尔结果读取节点输出可用性，适合只需要过滤候选项的调用方。 */
export function isWorkflowNodeOutputAvailable(
  args: WorkflowNodeOutputAvailabilityArgs,
): boolean {
  return getWorkflowNodeOutputAvailability(args).available;
}

type WorkflowGraph = Readonly<{
  /** 每个节点的有效前驱节点。 */
  predecessors: ReadonlyMap<string, ReadonlyArray<string>>;
  /** 从当前图入口可达的节点。 */
  reachable: ReadonlySet<string>;
  /** 用于支配计算的入口节点集合。 */
  entryIds: ReadonlySet<string>;
}>;

/** 建立只包含已知节点端点的编辑器图索引。 */
function buildGraph(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  edges: ReadonlyArray<WorkflowCanvasEdge>,
): WorkflowGraph {
  const nodeIds = new Set(nodes.map((node) => node.id));
  const predecessors = new Map<string, string[]>();
  const successors = new Map<string, string[]>();

  for (const nodeId of nodeIds) {
    predecessors.set(nodeId, []);
    successors.set(nodeId, []);
  }

  for (const edge of edges) {
    const sourceId = edge.source.nodeId;
    const targetId = edge.target.nodeId;
    if (!nodeIds.has(sourceId) || !nodeIds.has(targetId)) continue;
    predecessors.get(targetId)?.push(sourceId);
    successors.get(sourceId)?.push(targetId);
  }

  const entryIds = resolveEntryIds(nodes, predecessors);
  return {
    predecessors,
    reachable: collectReachable(entryIds, successors),
    entryIds,
  };
}

/** 选择与 Runtime 对齐的 Start 入口，缺少唯一 Start 时回退到结构入口。 */
function resolveEntryIds(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  predecessors: ReadonlyMap<string, ReadonlyArray<string>>,
): ReadonlySet<string> {
  const startIds = nodes
    .filter((node) => node.data.kind === 'start')
    .map((node) => node.id);
  if (startIds.length === 1) return new Set(startIds);

  return new Set(nodes
    .map((node) => node.id)
    .filter((nodeId) => (predecessors.get(nodeId)?.length ?? 0) === 0));
}

/** 沿正向邻接表收集从入口可达的节点。 */
function collectReachable(
  entryIds: ReadonlySet<string>,
  successors: ReadonlyMap<string, ReadonlyArray<string>>,
): ReadonlySet<string> {
  const reachable = new Set<string>();
  const queue = [...entryIds];
  /** 使用游标而不是 shift，保证循环图上的队列消费保持线性复杂度。 */
  let queueIndex = 0;
  while (queueIndex < queue.length) {
    const nodeId = queue[queueIndex];
    queueIndex += 1;
    if (nodeId === undefined || reachable.has(nodeId)) continue;
    reachable.add(nodeId);
    queue.push(...(successors.get(nodeId) ?? []));
  }
  return reachable;
}

/** 使用前驱交集不动点算法计算当前可达子图的支配集合。 */
function computeDominators(
  reachable: ReadonlySet<string>,
  predecessors: ReadonlyMap<string, ReadonlyArray<string>>,
  entryIds: ReadonlySet<string>,
): ReadonlyMap<string, ReadonlySet<string>> {
  const dominators = new Map<string, Set<string>>();
  for (const nodeId of reachable) {
    dominators.set(
      nodeId,
      entryIds.has(nodeId) ? new Set([nodeId]) : new Set(reachable),
    );
  }

  let changed = true;
  while (changed) {
    changed = false;
    for (const nodeId of reachable) {
      if (entryIds.has(nodeId)) continue;

      const nodePredecessors = (predecessors.get(nodeId) ?? [])
        .filter((predecessorId) => reachable.has(predecessorId));
      const next = intersectDominatorSets(nodePredecessors, dominators);
      next.add(nodeId);
      const current = dominators.get(nodeId);
      if (!sameSet(current, next)) {
        dominators.set(nodeId, next);
        changed = true;
      }
    }
  }

  return dominators;
}

/** 交集得到一个节点的全部必经前置节点。 */
function intersectDominatorSets(
  predecessorIds: ReadonlyArray<string>,
  dominators: ReadonlyMap<string, ReadonlySet<string>>,
): Set<string> {
  const firstPredecessor = predecessorIds[0];
  if (firstPredecessor === undefined) return new Set();

  const firstDominators = dominators.get(firstPredecessor);
  if (!firstDominators) return new Set();

  return predecessorIds.slice(1).reduce(
    (intersection, predecessorId) => {
      const candidate = dominators.get(predecessorId);
      if (!candidate) return new Set<string>();
      return new Set([...intersection].filter((id) => candidate.has(id)));
    },
    new Set(firstDominators),
  );
}

/** 比较两个只读集合，避免依赖集合的迭代顺序。 */
function sameSet(
  current: ReadonlySet<string> | undefined,
  next: ReadonlySet<string>,
): boolean {
  return current?.size === next.size
    && current !== undefined
    && [...current].every((id) => next.has(id));
}

/** 构造一个包含明确诊断文本的不可用结果。 */
function unavailable(unavailableReason: string): WorkflowSymbolAvailability {
  return { available: false, unavailableReason };
}
