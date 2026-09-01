import type {
  ComponentValueOutput,
  FlowComponentDefinition,
  ValueExpr,
  WorkflowInputDefinition,
  WorkflowNodeContract,
} from '../model/contracts';
import type { FlowComponentCatalogItem } from './componentCatalog';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
} from '../model/workflowModel';
import { encodeNodeDefinition } from '../model/workflowNodeDefinitions';

/** 从选择创建组件后的完整原子编辑结果。 */
export type ComponentCreationResult = Readonly<{
  catalogItem: FlowComponentCatalogItem;
  nodes: WorkflowCanvasNode[];
  edges: WorkflowCanvasEdge[];
  componentNodeId: string;
}>;

/** 连续选择不能形成 P0 单入口/单出口组件时的用户错误。 */
export class ComponentCreationError extends Error {}

/**
 * 把连续选择提取为带显式值输入/输出的版本锁定组件，并原地折叠实例。
 *
 * P0 不公开 resource ports，因此选择内部引用外部资源时明确拒绝。
 */
export function createComponentFromSelection(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  edges: ReadonlyArray<WorkflowCanvasEdge>,
  selectedNodeIds: ReadonlySet<string>,
  name: string,
  version: string,
): ComponentCreationResult {
  const normalizedName = name.trim();
  if (!normalizedName) throw new ComponentCreationError('请填写组合步骤名称。');
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new ComponentCreationError('版本号请使用“主版本.次版本.修订号”格式。');
  }
  const selectedNodes = nodes.filter((node) => selectedNodeIds.has(node.id));
  if (selectedNodes.length === 0) throw new ComponentCreationError('请先选择要保存的节点。');
  if (selectedNodes.some((node) => node.kind === 'start' || node.kind === 'end')) {
    throw new ComponentCreationError('请选择开始和结束之间的步骤。');
  }
  if (selectedNodes.some((node) => node.kind === 'loop')) {
    throw new ComponentCreationError('请先进入 While，再分别保存其中的普通步骤。');
  }
  const incomingEdges = edges.filter((edge) => (
    !selectedNodeIds.has(edge.source.nodeId) && selectedNodeIds.has(edge.target.nodeId)
  ));
  const outgoingEdges = edges.filter((edge) => (
    selectedNodeIds.has(edge.source.nodeId) && !selectedNodeIds.has(edge.target.nodeId)
  ));
  if (incomingEdges.length !== 1 || outgoingEdges.length !== 1) {
    throw new ComponentCreationError('所选步骤需要一个入口和一个出口。请调整选择范围。');
  }
  assertNoExternalResourceReferences(selectedNodes, selectedNodeIds);

  const componentId = crypto.randomUUID();
  const componentNodeId = `component-${crypto.randomUUID()}`;
  const inputBindings = collectExternalValueInputs(selectedNodes, selectedNodeIds);
  const outputBindings = collectConsumedOutputs(nodes, selectedNodeIds);
  const minX = Math.min(...selectedNodes.map((node) => node.position.x));
  const minY = Math.min(...selectedNodes.map((node) => node.position.y));
  const maxX = Math.max(...selectedNodes.map((node) => node.position.x + node.size.width));
  const entryNodeId = 'component_entry';
  const exitNodeId = 'component_exit';
  const internalNodes = selectedNodes.map((node) => encodeInternalNode(
    node,
    minX,
    minY,
    inputBindings,
  ));
  internalNodes.push(boundaryNode(entryNodeId, 0, incomingEdges[0].target.nodeId, internalNodes));
  internalNodes.push(boundaryNode(exitNodeId, maxX - minX + 220, null, internalNodes));
  const definition: FlowComponentDefinition = {
    schema_version: 2,
    id: componentId,
    version,
    name: normalizedName,
    inputs: inputBindings.map(({ name: key }) => ({
      key,
      value_type: 'text',
    } satisfies WorkflowInputDefinition)),
    outputs: outputBindings.map(({ name: outputName, original }) => ({
      name: outputName,
      value: original,
    } satisfies ComponentValueOutput)),
    graph: {
      root_scope_id: 'component_root',
      scopes: [{
        id: 'component_root',
        parent: null,
        boundary: {
          type: 'component',
          entry_node_id: entryNodeId,
          exit_node_id: exitNodeId,
        },
        nodes: internalNodes,
        edges: buildInternalEdges(
          edges,
          selectedNodeIds,
          incomingEdges[0].target.nodeId,
          outgoingEdges[0].source.nodeId,
          entryNodeId,
          exitNodeId,
        ),
      }],
    },
  };
  const componentData: Extract<WorkflowNodeData, { kind: 'component' }> = {
    kind: 'component',
    label: normalizedName,
    outputBindings: {},
    component: {
      component_id: componentId,
      component_version: version,
      inputs: Object.fromEntries(inputBindings.map((input) => [input.name, input.original])),
    },
    componentName: normalizedName,
    componentOutputs: definition.outputs,
    componentDefinition: definition,
    runState: 'idle',
  };
  const componentNode: WorkflowCanvasNode = {
    id: componentNodeId,
    kind: 'component',
    position: { x: minX, y: minY },
    size: { ...WORKFLOW_NODE_SIZES.component },
    data: componentData,
  };
  const remainingNodes = nodes
    .filter((node) => !selectedNodeIds.has(node.id))
    .map((node) => rewriteNodeOutputReferences(node, outputBindings, componentNodeId));
  const nextEdges = edges
    .filter((edge) => (
      !selectedNodeIds.has(edge.source.nodeId)
      && !selectedNodeIds.has(edge.target.nodeId)
    ))
    .concat([
      {
        ...incomingEdges[0],
        target: { ...incomingEdges[0].target, nodeId: componentNodeId },
      },
      {
        ...outgoingEdges[0],
        source: { ...outgoingEdges[0].source, nodeId: componentNodeId },
      },
    ]);
  return {
    catalogItem: {
      title: normalizedName,
      description: `保存的组合步骤 · ${version}`,
      definition,
      defaultInputs: componentData.component.inputs,
      valueOutputs: definition.outputs.map((output) => ({
        name: output.name,
        valueType: 'json',
        label: output.name,
      })),
    },
    nodes: [...remainingNodes, componentNode],
    edges: nextEdges,
    componentNodeId,
  };
}

type ExternalInput = Readonly<{
  name: string;
  key: string;
  original: ValueExpr;
}>;

type ComponentOutput = Readonly<{
  name: string;
  key: string;
  original: ValueExpr;
}>;

/** 把画布节点编码为组件内部开放契约，并替换父流程值输入。 */
function encodeInternalNode(
  node: WorkflowCanvasNode,
  minX: number,
  minY: number,
  inputs: ReadonlyArray<ExternalInput>,
): WorkflowNodeContract {
  const definition = encodeNodeDefinition(node.data);
  const contract: WorkflowNodeContract = {
    id: node.id,
    position: {
      x: node.position.x - minX + 180,
      y: node.position.y - minY + 80,
    },
    size: node.size,
    output_bindings: node.data.outputBindings,
    ...definition,
  };
  return rewriteValue(contract, (expression) => {
    const key = valueExprKey(expression);
    const input = inputs.find((candidate) => candidate.key === key);
    return input
      ? {
          type: 'ref',
          source: { type: 'workflow_input', key: input.name },
          pointer: '',
        }
      : expression;
  });
}

/** 创建组件唯一入口或出口边界节点。 */
function boundaryNode(
  id: string,
  x: number,
  entryTargetId: string | null,
  nodes: ReadonlyArray<WorkflowNodeContract>,
): WorkflowNodeContract {
  const target = entryTargetId
    ? nodes.find((node) => node.id === entryTargetId)
    : nodes.at(-1);
  return {
    id,
    position: { x, y: target?.position.y ?? 80 },
    size: entryTargetId
      ? { ...WORKFLOW_NODE_SIZES.start }
      : { ...WORKFLOW_NODE_SIZES.end },
    type_id: entryTargetId ? 'argus.start' : 'argus.end',
    version: 1,
    payload: {},
    output_bindings: {},
  };
}

/** 复制选择内部边并连接合成的唯一边界节点。 */
function buildInternalEdges(
  edges: ReadonlyArray<WorkflowCanvasEdge>,
  selectedNodeIds: ReadonlySet<string>,
  entryTargetId: string,
  exitSourceId: string,
  entryNodeId: string,
  exitNodeId: string,
): import('../model/contracts').WorkflowEdgeContract[] {
  const internal = edges
    .filter((edge) => (
      selectedNodeIds.has(edge.source.nodeId)
      && selectedNodeIds.has(edge.target.nodeId)
    ))
    .map((edge) => ({
      id: edge.id,
      source: edge.source.nodeId,
      target: edge.target.nodeId,
      branch: edge.data.branch,
    }));
  return [
    {
      id: `edge_${entryNodeId}_${entryTargetId}`,
      source: entryNodeId,
      target: entryTargetId,
      branch: null,
    },
    ...internal,
    {
      id: `edge_${exitSourceId}_${exitNodeId}`,
      source: exitSourceId,
      target: exitNodeId,
      branch: null,
    },
  ];
}

/** 收集选择内部引用的父流程值，并给每个完整表达式分配稳定输入名。 */
function collectExternalValueInputs(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  selectedNodeIds: ReadonlySet<string>,
): ExternalInput[] {
  const expressions = new Map<string, ValueExpr>();
  for (const node of nodes) {
    visitValue(serializedNodeInputs(node), (expression) => {
      if (expression.type !== 'ref'
        || (expression.source.type === 'node' && selectedNodeIds.has(expression.source.node_id))) {
        return;
      }
      expressions.set(valueExprKey(expression), expression);
    });
  }
  return [...expressions.entries()].map(([key, original], index) => ({
    name: `input_${index + 1}`,
    key,
    original,
  }));
}

/** 收集选择外节点实际消费的内部输出，避免公开未使用端口。 */
function collectConsumedOutputs(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  selectedNodeIds: ReadonlySet<string>,
): ComponentOutput[] {
  const expressions = new Map<string, ValueExpr>();
  for (const node of nodes.filter((candidate) => !selectedNodeIds.has(candidate.id))) {
    visitValue(serializedNodeInputs(node), (expression) => {
      if (expression.type === 'ref'
        && expression.source.type === 'node'
        && selectedNodeIds.has(expression.source.node_id)) {
        expressions.set(valueExprKey(expression), expression);
      }
    });
  }
  return [...expressions.entries()].map(([key, original], index) => ({
    name: `output_${index + 1}`,
    key,
    original,
  }));
}

/** P0 不允许组件通过内部 ResourceRef 捕获父流程资源。 */
function assertNoExternalResourceReferences(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  selectedNodeIds: ReadonlySet<string>,
) {
  for (const node of nodes) {
    visitObject(serializedNodeInputs(node), (object) => {
      const producerNodeId = object.producer_node_id;
      if (typeof producerNodeId === 'string' && !selectedNodeIds.has(producerNodeId)) {
        throw new ComponentCreationError('所选步骤使用了范围外的应用或浏览器。请把对应节点一起选中。');
      }
    });
  }
}

/** 把外部节点对内部输出的引用改写为组件公开端口。 */
function rewriteNodeOutputReferences(
  node: WorkflowCanvasNode,
  outputs: ReadonlyArray<ComponentOutput>,
  componentNodeId: string,
): WorkflowCanvasNode {
  const data = rewriteValue(node.data, (expression) => {
    const output = outputs.find((candidate) => candidate.key === valueExprKey(expression));
    return output
      ? {
          type: 'ref',
          source: { type: 'node', node_id: componentNodeId },
          pointer: `/${output.name}`,
        }
      : expression;
  });
  return {
    ...node,
    data: node.data.kind === 'component' && data.kind === 'component'
      ? { ...data, componentDefinition: node.data.componentDefinition }
      : data,
  };
}

/** 只暴露节点实际编码 payload 和公开输出，不扫描编辑器展示元数据。 */
function serializedNodeInputs(node: WorkflowCanvasNode): unknown {
  const definition = encodeNodeDefinition(node.data);
  return {
    payload: definition.payload,
    output_bindings: node.data.outputBindings,
  };
}

/** 遍历未知对象中结构完整的 ValueExpr。 */
function visitValue(value: unknown, visit: (expression: ValueExpr) => void) {
  if (!value || typeof value !== 'object') return;
  if (isValueExpr(value)) visit(value);
  for (const child of Object.values(value)) visitValue(child, visit);
}

/** 遍历未知对象，供 ResourceRef 捕获检查复用。 */
function visitObject(value: unknown, visit: (object: Record<string, unknown>) => void) {
  if (!value || typeof value !== 'object') return;
  if (!Array.isArray(value)) visit(value as Record<string, unknown>);
  for (const child of Object.values(value)) visitObject(child, visit);
}

/** 递归复制数据并替换所有 ValueExpr。 */
function rewriteValue<T>(
  value: T,
  rewrite: (expression: ValueExpr) => ValueExpr,
): T {
  if (!value || typeof value !== 'object') return value;
  if (isValueExpr(value)) return rewrite(value) as T;
  if (Array.isArray(value)) {
    return value.map((child) => rewriteValue(child, rewrite)) as T;
  }
  return Object.fromEntries(Object.entries(value).map(([key, child]) => (
    [key, rewriteValue(child, rewrite)]
  ))) as T;
}

/** 使用判别值和必需字段识别 ValueExpr。 */
function isValueExpr(value: object): value is ValueExpr {
  const candidate = value as Partial<ValueExpr>;
  return candidate.type === 'literal'
    || candidate.type === 'expression'
    || (candidate.type === 'ref' && 'source' in candidate);
}

/** 将结构化表达式序列化为稳定去重键。 */
function valueExprKey(expression: ValueExpr): string {
  return JSON.stringify(expression);
}
