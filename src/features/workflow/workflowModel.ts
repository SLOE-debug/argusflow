import type { FlowEdge, FlowNode, FlowPoint } from '../../flow/types';
import type {
  ApplicationSpec,
  BrowserSpec,
  CommandOperation,
  ControlPortId,
  ConditionOperator,
  ExecutionEvent,
  JsonObject,
  UiOperation,
  ValueExpr,
  VariableAssignment,
  WorkflowDefinition,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  createRegisteredNodeData,
  encodeNodeDefinition,
  isUnaryCondition,
} from './workflowNodeDefinitions';

/** 节点在单次工作流运行中的展示状态。 */
export type NodeRunState =
  | 'idle'
  | 'pending'
  | 'running'
  | 'success'
  | 'error'
  | 'skipped';

/** 所有工作流节点共享的编辑器状态。 */
type WorkflowNodeDataBase = {
  label: string;
  /** 所有节点共享的 Published Outputs 自定义映射。 */
  outputBindings: Readonly<Record<string, ValueExpr>>;
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
      left: ValueExpr;
      operator: ConditionOperator;
      right: ValueExpr | null;
    }
  | WorkflowNodeDataBase & { kind: 'variable'; assignments: VariableAssignment[] }
  | WorkflowNodeDataBase & { kind: 'application'; spec: ApplicationSpec }
  | WorkflowNodeDataBase & { kind: 'browser'; spec: BrowserSpec }
  | WorkflowNodeDataBase & { kind: 'ui'; operation: UiOperation }
  | WorkflowNodeDataBase & { kind: 'command'; operation: CommandOperation }
  | WorkflowNodeDataBase & { kind: 'end' };

/** 可由节点库创建的完整节点类型集合。 */
export type EditableNodeKind = WorkflowNodeData['kind'];

/** 以不可变方式更新一个节点判别联合。 */
export type WorkflowNodeUpdater = (current: WorkflowNodeData) => WorkflowNodeData;

export type WorkflowEdgeData = { branch: ControlPortId | null };
export type WorkflowCanvasNode = FlowNode<WorkflowNodeData>;
export type WorkflowCanvasEdge = FlowEdge<WorkflowEdgeData>;

/** 工作流节点在高密度桌面画布中的统一尺寸。 */
export const WORKFLOW_NODE_SIZES = {
  start: { width: 118, height: 52 },
  log: { width: 142, height: 52 },
  debug: { width: 156, height: 52 },
  delay: { width: 136, height: 52 },
  condition: { width: 132, height: 52 },
  variable: { width: 148, height: 52 },
  application: { width: 172, height: 52 },
  browser: { width: 172, height: 52 },
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
  let branch: ControlPortId | null = null;
  if (sourceNode?.kind === 'condition') {
    const used = new Set(edges.filter((edge) => edge.source.nodeId === source).map((edge) => edge.data.branch));
    branch = used.has('true') ? 'false' : 'true';
  }
  return { id: `edge-${crypto.randomUUID()}`, source: { nodeId: source, side: sourceSide }, target: { nodeId: target, side: targetSide }, data: { branch } };
}

/** 将画布状态转换为后端 schema v8 开放节点契约。 */
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
    schema_version: 8,
    id: workflowId,
    name,
    inputs: [...inputs],
    variables,
    permissions,
    nodes: nodes.map((node) => ({
      id: node.id,
      position: node.position,
      output_bindings: node.data.outputBindings,
      ...encodeNodeDefinition(node.data),
    })),
    edges: edges.map((edge) => ({ id: edge.id, source: edge.source.nodeId, target: edge.target.nodeId, branch: edge.data.branch })),
  };
}

/** 根据后端事件更新对应节点状态。 */
export function applyExecutionEventToNodes(nodes: ReadonlyArray<WorkflowCanvasNode>, event: ExecutionEvent): WorkflowCanvasNode[] {
  if (event.kind === 'workflow_started') {
    return nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        runState: !node.data.runState || node.data.runState === 'idle'
          ? 'pending'
          : node.data.runState,
        invalid: false,
      },
    }));
  }
  if (
    event.kind === 'workflow_completed'
    || event.kind === 'workflow_failed'
  ) {
    return nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        runState: node.data.runState === 'pending'
          ? 'skipped'
          : node.data.runState,
      },
    }));
  }
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

export const isUnary = isUnaryCondition;

/** 为每种节点建立字段完整且立即可编辑的默认数据。 */
function createNodeData(kind: EditableNodeKind): WorkflowNodeData {
  return createRegisteredNodeData(kind);
}
