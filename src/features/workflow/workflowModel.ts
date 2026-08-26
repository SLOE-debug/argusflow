import type { FlowEdge, FlowNode, FlowPoint } from '../../flow/types';
import type {
  ApplicationSpec,
  CommandOperation,
  ConditionBranch,
  ConditionOperator,
  ExecutionEvent,
  JsonObject,
  JsonValue,
  UiOperation,
  ValueExpr,
  WorkflowDefinition,
  WorkflowInputDefinition,
  WorkflowNodeKind,
  WorkflowPermissions,
} from './contracts';
import { createDefaultUiOperation } from './workflowAction';
import { createDefaultApplicationSpec } from './workflowApplication';
import { createDefaultCommandOperation } from './workflowCommand';

export type NodeRunState = 'idle' | 'running' | 'success' | 'error';

/** 所有工作流节点共享的编辑器状态。 */
type WorkflowNodeDataBase = {
  label: string;
  runState?: NodeRunState;
  invalid?: boolean;
};

/** ArgusFlow 节点在通用 Flow 内核中保存的强类型业务字段。 */
export type WorkflowNodeData =
  | WorkflowNodeDataBase & { kind: 'start' }
  | WorkflowNodeDataBase & { kind: 'log'; message: string }
  | WorkflowNodeDataBase & { kind: 'debug'; value: ValueExpr }
  | WorkflowNodeDataBase & { kind: 'delay'; milliseconds: number }
  | WorkflowNodeDataBase & {
      kind: 'condition';
      pointer: string;
      operator: ConditionOperator;
      operand: JsonValue;
    }
  | WorkflowNodeDataBase & { kind: 'application'; spec: ApplicationSpec }
  | WorkflowNodeDataBase & { kind: 'ui'; operation: UiOperation }
  | WorkflowNodeDataBase & { kind: 'command'; operation: CommandOperation }
  | WorkflowNodeDataBase & { kind: 'end' };

/** 可由节点库创建的完整节点类型集合。 */
export type EditableNodeKind = WorkflowNodeData['kind'];

/** 以不可变方式更新一个节点判别联合。 */
export type WorkflowNodeUpdater = (current: WorkflowNodeData) => WorkflowNodeData;

export type WorkflowEdgeData = { branch: ConditionBranch | null };
export type WorkflowCanvasNode = FlowNode<WorkflowNodeData>;
export type WorkflowCanvasEdge = FlowEdge<WorkflowEdgeData>;

/** 工作流节点在高密度桌面画布中的统一尺寸。 */
export const WORKFLOW_NODE_SIZES = {
  start: { width: 118, height: 52 },
  log: { width: 142, height: 52 },
  debug: { width: 156, height: 52 },
  delay: { width: 136, height: 52 },
  condition: { width: 132, height: 52 },
  application: { width: 172, height: 52 },
  ui: { width: 164, height: 52 },
  command: { width: 164, height: 52 },
  end: { width: 122, height: 52 },
} as const satisfies Readonly<
  Record<EditableNodeKind, Readonly<{ width: number; height: number }>>
>;

/** 在指定世界坐标创建一个业务节点。 */
export function createNode(kind: EditableNodeKind, position: FlowPoint = { x: 200, y: 160 }): WorkflowCanvasNode {
  return {
    id: `${kind}-${crypto.randomUUID()}`,
    kind,
    position: {
      x: Math.round(position.x),
      y: Math.round(position.y),
    },
    size: { ...WORKFLOW_NODE_SIZES[kind] },
    data: createNodeData(kind),
  };
}

/** 新增边并根据 Condition 已占分支自动分配 true/false。 */
export function createEdge(source: string, target: string, nodes: ReadonlyArray<WorkflowCanvasNode>, edges: ReadonlyArray<WorkflowCanvasEdge>, sourceSide?: WorkflowCanvasEdge['source']['side'], targetSide?: WorkflowCanvasEdge['target']['side']): WorkflowCanvasEdge {
  const sourceNode = nodes.find((node) => node.id === source);
  let branch: ConditionBranch | null = null;
  if (sourceNode?.kind === 'condition') {
    const used = new Set(edges.filter((edge) => edge.source.nodeId === source).map((edge) => edge.data.branch));
    branch = used.has('true') ? 'false' : 'true';
  }
  return { id: `edge-${crypto.randomUUID()}`, source: { nodeId: source, side: sourceSide }, target: { nodeId: target, side: targetSide }, data: { branch } };
}

/** 将画布状态转换为后端 schema v5 契约。 */
export function toWorkflowDefinition(
  workflowId: string,
  name: string,
  inputs: ReadonlyArray<WorkflowInputDefinition>,
  variables: JsonObject,
  permissions: WorkflowPermissions,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  edges: ReadonlyArray<WorkflowCanvasEdge>,
): WorkflowDefinition {
  return {
    schema_version: 5,
    id: workflowId,
    name,
    inputs: [...inputs],
    variables,
    permissions,
    nodes: nodes.map((node) => ({ id: node.id, position: node.position, ...toNodeKind(node.data) })),
    edges: edges.map((edge) => ({ id: edge.id, source: edge.source.nodeId, target: edge.target.nodeId, branch: edge.data.branch })),
  };
}

/** 根据后端事件更新对应节点状态。 */
export function applyExecutionEventToNodes(nodes: ReadonlyArray<WorkflowCanvasNode>, event: ExecutionEvent): WorkflowCanvasNode[] {
  if (event.kind === 'workflow_started') return nodes.map((node) => ({ ...node, data: { ...node.data, runState: 'idle', invalid: false } }));
  const runState = event.kind === 'node_started' ? 'running' : event.kind === 'node_succeeded' ? 'success' : event.kind === 'node_failed' ? 'error' : null;
  if (!event.node_id || !runState) return [...nodes];
  return nodes.map((node) => node.id === event.node_id ? { ...node, data: { ...node.data, runState } } : node);
}

/** 检查新增或重连后的有向图约束。 */
export function canConnect(nodes: ReadonlyArray<WorkflowCanvasNode>, edges: ReadonlyArray<WorkflowCanvasEdge>, source: string, target: string, ignoredEdgeId?: string): boolean {
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
    case 'log': return { type: 'log', message: data.message };
    case 'debug': return { type: 'debug', value: data.value };
    case 'delay': return { type: 'delay', milliseconds: data.milliseconds };
    case 'condition': return { type: 'condition', predicate: { pointer: data.pointer, operator: data.operator, operand: isUnary(data.operator) ? null : data.operand } };
    case 'application': return { type: 'application', spec: data.spec };
    case 'ui': return { type: 'ui', operation: data.operation };
    case 'command': return { type: 'command', operation: data.operation };
    case 'end': return { type: 'end' };
  }
}

export const isUnary = (operator?: ConditionOperator): boolean => operator === 'exists' || operator === 'not_exists' || operator === 'is_empty' || operator === 'not_empty';

/** 为每种节点建立字段完整且立即可编辑的默认数据。 */
function createNodeData(kind: EditableNodeKind): WorkflowNodeData {
  switch (kind) {
    case 'start':
      return { kind, label: '开始', runState: 'idle' };
    case 'log':
      return { kind, label: '日志', message: '记录一条运行信息', runState: 'idle' };
    case 'debug':
      return {
        kind,
        label: '调试输出',
        value: { type: 'literal', value: '' },
        runState: 'idle',
      };
    case 'delay':
      return { kind, label: '等待', milliseconds: 500, runState: 'idle' };
    case 'condition':
      return {
        kind,
        label: '条件',
        pointer: '/enabled',
        operator: 'equal',
        operand: true,
        runState: 'idle',
      };
    case 'application':
      return {
        kind,
        label: '打开或连接应用',
        spec: createDefaultApplicationSpec(),
        runState: 'idle',
      };
    case 'ui':
      return {
        kind,
        label: '界面操作',
        operation: createDefaultUiOperation(),
        runState: 'idle',
      };
    case 'command':
      return {
        kind,
        label: '执行命令',
        operation: createDefaultCommandOperation(),
        runState: 'idle',
      };
    case 'end':
      return { kind, label: '结束', runState: 'idle' };
  }
}
