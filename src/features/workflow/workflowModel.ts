import type { FlowEdge, FlowNode, FlowPoint } from '../../flow/types';
import type {
  ApplicationSpec,
  BrowserSpec,
  BrowserOperation,
  CommandOperation,
  ComponentInstance,
  ComponentValueOutput,
  ControlPortId,
  ConditionOperator,
  ExecutionEvent,
  JsonObject,
  DelimitedTextFormat,
  UiOperation,
  UiExecutionPolicy,
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
import {
  findFlowComponent,
  FLOW_COMPONENT_CATALOG,
  type FlowComponentCatalogItem,
} from './componentCatalog';
import { findNodePreset } from './nodePresetCatalog';

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
  | WorkflowNodeDataBase & { kind: 'navigate'; operation: BrowserOperation }
  | WorkflowNodeDataBase & {
      kind: 'ui';
      operation: UiOperation;
      execution: UiExecutionPolicy;
      /** 可选节点预设来源；编码时仍保存为普通 argus.ui。 */
      presetId?: string;
    }
  | WorkflowNodeDataBase & { kind: 'command'; operation: CommandOperation }
  | WorkflowNodeDataBase & { kind: 'format'; operation: DelimitedTextFormat }
  | WorkflowNodeDataBase & {
      kind: 'component';
      component: ComponentInstance;
      componentName: string;
      componentOutputs: ReadonlyArray<ComponentValueOutput>;
      /** Studio 下钻和本次运行注册使用的精确冻结定义。 */
      componentDefinition: import('./contracts').FlowComponentDefinition;
    }
  | WorkflowNodeDataBase & { kind: 'end' };

/** 可由节点库创建的完整节点类型集合。 */
export type EditableNodeKind = WorkflowNodeData['kind'];

/** 节点库拖放使用的稳定创建键；Preset/Component 与画布 kind 分离。 */
export type WorkflowNodeCreationKey =
  | EditableNodeKind
  | `preset:${string}`
  | `component:${string}@${string}`;

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
  navigate: { width: 164, height: 52 },
  ui: { width: 164, height: 52 },
  command: { width: 164, height: 52 },
  format: { width: 164, height: 52 },
  component: { width: 188, height: 58 },
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

/** 根据 Primitive、Preset 或 Component 创建键建立最终画布节点。 */
export function createNodeFromCreationKey(
  creationKey: WorkflowNodeCreationKey,
  position: FlowPoint = { x: 200, y: 160 },
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem> = FLOW_COMPONENT_CATALOG,
): WorkflowCanvasNode | null {
  const kind = resolveCreationKind(creationKey);
  if (!kind) return null;
  const node = createNode(kind, position);
  if (creationKey.startsWith('preset:')) {
    const preset = findNodePreset(creationKey.slice('preset:'.length));
    if (!preset || node.data.kind !== 'ui') return null;
    return {
      ...node,
      data: {
        ...node.data,
        label: preset.label,
        operation: preset.operation(),
        execution: preset.execution(),
        presetId: preset.id,
      },
    };
  }
  if (creationKey.startsWith('component:')) {
    const reference = creationKey.slice('component:'.length);
    const separatorIndex = reference.lastIndexOf('@');
    if (separatorIndex < 1 || node.data.kind !== 'component') return null;
    const componentId = reference.slice(0, separatorIndex);
    const componentVersion = reference.slice(separatorIndex + 1);
    const item = findFlowComponent(componentId, componentVersion, componentCatalog);
    if (!item) return null;
    return {
      ...node,
      data: {
        ...node.data,
        label: item.title,
        componentName: item.definition.name,
        component: {
          component_id: item.definition.id,
          component_version: item.definition.version,
          inputs: item.defaultInputs,
        },
        componentOutputs: item.definition.outputs,
        componentDefinition: item.definition,
      },
    };
  }
  return node;
}

/** 解析创建键实际占用的画布节点注册 kind。 */
export function resolveCreationKind(creationKey: string): EditableNodeKind | null {
  if (creationKey.startsWith('preset:')) {
    return findNodePreset(creationKey.slice('preset:'.length)) ? 'ui' : null;
  }
  if (creationKey.startsWith('component:')) {
    const reference = creationKey.slice('component:'.length);
    const separatorIndex = reference.lastIndexOf('@');
    if (separatorIndex < 1) return null;
    return separatorIndex > 0 ? 'component' : null;
  }
  return Object.hasOwn(WORKFLOW_NODE_SIZES, creationKey)
    ? creationKey as EditableNodeKind
    : null;
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
