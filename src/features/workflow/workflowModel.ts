import type { FlowEdge, FlowNode, FlowPoint } from '../../flow/types';
import type { ConditionBranch, ConditionOperator, ExecutionEvent, JsonObject, JsonValue, WorkflowDefinition, WorkflowNodeKind } from './contracts';

export type EditableNodeKind = 'start' | 'log' | 'delay' | 'condition' | 'end';
export type NodeRunState = 'idle' | 'running' | 'success' | 'error';

/** ArgusFlow 节点在通用 Flow 内核中保存的业务字段。 */
export type WorkflowNodeData = {
  kind: EditableNodeKind;
  label: string;
  message?: string;
  milliseconds?: number;
  pointer?: string;
  operator?: ConditionOperator;
  operand?: JsonValue;
  runState?: NodeRunState;
  invalid?: boolean;
};

export type WorkflowEdgeData = { branch: ConditionBranch | null };
export type WorkflowCanvasNode = FlowNode<WorkflowNodeData>;
export type WorkflowCanvasEdge = FlowEdge<WorkflowEdgeData>;

/** 编辑器以完全空白文档开始。 */
export const DEFAULT_NODES: WorkflowCanvasNode[] = [];
export const DEFAULT_EDGES: WorkflowCanvasEdge[] = [];

const NODE_DEFAULTS: Record<EditableNodeKind, { label: string; size: { width: number; height: number }; extras?: Partial<WorkflowNodeData> }> = {
  start: { label: '开始', size: { width: 168, height: 68 } },
  log: { label: '日志', size: { width: 200, height: 72 }, extras: { message: '记录一条运行信息' } },
  delay: { label: '等待', size: { width: 200, height: 72 }, extras: { milliseconds: 500 } },
  condition: { label: '条件', size: { width: 200, height: 72 }, extras: { pointer: '/enabled', operator: 'equal', operand: true } },
  end: { label: '结束', size: { width: 168, height: 68 } },
};

/** 在指定世界坐标创建一个业务节点。 */
export function createNode(kind: EditableNodeKind, position: FlowPoint = { x: 200, y: 160 }): WorkflowCanvasNode {
  const defaults = NODE_DEFAULTS[kind];
  return {
    id: `${kind}-${crypto.randomUUID()}`,
    kind,
    position,
    size: defaults.size,
    data: { kind, label: defaults.label, runState: 'idle', ...defaults.extras },
  };
}

/** 新增边并根据 Condition 已占分支自动分配 true/false。 */
export function createEdge(source: string, target: string, nodes: WorkflowCanvasNode[], edges: WorkflowCanvasEdge[], sourceSide?: WorkflowCanvasEdge['source']['side'], targetSide?: WorkflowCanvasEdge['target']['side']): WorkflowCanvasEdge {
  const sourceNode = nodes.find((node) => node.id === source);
  let branch: ConditionBranch | null = null;
  if (sourceNode?.kind === 'condition') {
    const used = new Set(edges.filter((edge) => edge.source.nodeId === source).map((edge) => edge.data.branch));
    branch = used.has('true') ? 'false' : 'true';
  }
  return { id: `edge-${crypto.randomUUID()}`, source: { nodeId: source, side: sourceSide }, target: { nodeId: target, side: targetSide }, data: { branch } };
}

/** 将画布状态转换为后端 schema v2 契约。 */
export function toWorkflowDefinition(workflowId: string, name: string, variables: JsonObject, nodes: WorkflowCanvasNode[], edges: WorkflowCanvasEdge[]): WorkflowDefinition {
  return {
    schema_version: 2,
    id: workflowId,
    name,
    variables,
    nodes: nodes.map((node) => ({ id: node.id, position: node.position, ...toNodeKind(node.data) })),
    edges: edges.map((edge) => ({ id: edge.id, source: edge.source.nodeId, target: edge.target.nodeId, branch: edge.data.branch })),
  };
}

/** 根据后端事件更新对应节点状态。 */
export function applyExecutionEventToNodes(nodes: WorkflowCanvasNode[], event: ExecutionEvent): WorkflowCanvasNode[] {
  if (event.kind === 'workflow_started') return nodes.map((node) => ({ ...node, data: { ...node.data, runState: 'idle', invalid: false } }));
  const runState = event.kind === 'node_started' ? 'running' : event.kind === 'node_succeeded' ? 'success' : event.kind === 'node_failed' ? 'error' : null;
  if (!event.node_id || !runState) return nodes;
  return nodes.map((node) => node.id === event.node_id ? { ...node, data: { ...node.data, runState } } : node);
}

/** 检查新增或重连后的有向图约束。 */
export function canConnect(nodes: WorkflowCanvasNode[], edges: WorkflowCanvasEdge[], source: string, target: string, ignoredEdgeId?: string): boolean {
  if (source === target || edges.some((edge) => edge.id !== ignoredEdgeId && edge.source.nodeId === source && edge.target.nodeId === target)) return false;
  const sourceNode = nodes.find((node) => node.id === source);
  const targetNode = nodes.find((node) => node.id === target);
  if (!sourceNode || !targetNode || sourceNode.kind === 'end' || targetNode.kind === 'start') return false;
  const outgoing = edges.filter((edge) => edge.id !== ignoredEdgeId && edge.source.nodeId === source).length;
  if (outgoing >= (sourceNode.kind === 'condition' ? 2 : 1)) return false;
  const adjacency = new Map<string, string[]>();
  for (const edge of edges.filter((edge) => edge.id !== ignoredEdgeId)) adjacency.set(edge.source.nodeId, [...(adjacency.get(edge.source.nodeId) ?? []), edge.target.nodeId]);
  const queue = [target];
  const visited = new Set<string>();
  while (queue.length > 0) {
    const id = queue.shift()!;
    if (id === source) return false;
    if (visited.has(id)) continue;
    visited.add(id);
    queue.push(...(adjacency.get(id) ?? []));
  }
  return true;
}

function toNodeKind(data: WorkflowNodeData): WorkflowNodeKind {
  switch (data.kind) {
    case 'start': return { type: 'start' };
    case 'log': return { type: 'log', message: data.message ?? '' };
    case 'delay': return { type: 'delay', milliseconds: data.milliseconds ?? 0 };
    case 'condition': return { type: 'condition', predicate: { pointer: data.pointer ?? '', operator: data.operator ?? 'equal', operand: isUnary(data.operator) ? null : data.operand ?? null } };
    case 'end': return { type: 'end' };
  }
}

export const isUnary = (operator?: ConditionOperator): boolean => operator === 'exists' || operator === 'not_exists' || operator === 'is_empty' || operator === 'not_empty';
