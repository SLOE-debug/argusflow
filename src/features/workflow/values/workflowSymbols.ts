import type {
  JsonObject,
  ValueExpr,
  ValueOutputDescriptor,
  WorkflowInputDefinition,
} from '../model/contracts';
import { getNodeValueOutputs } from '../model/workflowNodeDefinitions';
import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
} from '../model/workflowModel';
import {
  buildWorkflowNodeOutputAvailabilityIndex,
  type WorkflowSymbolAvailability,
} from './workflowSymbolAvailability';

/** 工作流输入在值选择器中的稳定展示符号。 */
export type WorkflowInputSymbol = Readonly<{
  /** 由输入名称组成的编辑器层稳定 ID。 */
  id: `input:${string}`;
  /** 符号来源类别。 */
  kind: 'workflow_input';
  /** 持久化 ValueSource 使用的输入 key。 */
  name: string;
  /** 值选择器显示名称；schema v9 暂无独立 label。 */
  label: string;
  /** 当前 schema v9 输入的值类型。 */
  valueType: 'text';
  /** 输入声明始终可作为引用候选。 */
  available: true;
}>;

/** 工作流变量在值选择器中的稳定展示符号。 */
export type WorkflowVariableSymbol = Readonly<{
  /** 由变量名称组成的编辑器层稳定 ID。 */
  id: `variable:${string}`;
  /** 符号来源类别。 */
  kind: 'variable';
  /** 持久化 ValueSource 使用的变量名称。 */
  name: string;
  /** 值选择器显示名称。 */
  label: string;
  /** 变量值允许在运行期改变 JSON category。 */
  valueType: 'json';
  /** 已声明变量始终可作为引用候选。 */
  available: true;
}>;

/** 节点 Published Output 在值选择器中的稳定展示符号。 */
export type WorkflowNodeOutputSymbol = Readonly<{
  /** 由节点 ID 与公开输出名称组成的编辑器层稳定 ID。 */
  id: `node:${string}:output:${string}`;
  /** 符号来源类别。 */
  kind: 'node_output';
  /** 公开输出所属的稳定节点 ID。 */
  nodeId: string;
  /** Published Output 的稳定名称。 */
  outputName: string;
  /** 统一搜索/预览契约使用的短名称，等同于 outputName。 */
  name: string;
  /** 生产节点的当前显示名称。 */
  nodeLabel: string;
  /** 值选择器中展示的节点与输出名称。 */
  label: string;
  /** Published Output 的编辑器展示类型。 */
  valueType: 'text' | 'json';
  /** 是否满足当前消费节点的控制流可用性约束。 */
  available: boolean;
  /** 输出不可用时的控制流诊断。 */
  unavailableReason?: string;
}>;

/** 节点完整 Published Outputs 对象在值选择器中的展示符号。 */
export type WorkflowNodeResultSymbol = Readonly<{
  /** 与具体 Published Output 分离的稳定编辑器 ID。 */
  id: `node:${string}:result`;
  /** 节点完整结果使用独立判别值，避免伪造输出名称。 */
  kind: 'node_result';
  /** 生产节点的稳定 ID。 */
  nodeId: string;
  /** 完整结果没有单独的 Published Output 名称。 */
  outputName: null;
  /** 统一搜索使用的节点稳定 ID。 */
  name: string;
  /** 生产节点的当前显示名称。 */
  nodeLabel: string;
  /** 值选择器中的可读名称。 */
  label: string;
  /** 节点完整结果始终是 JSON 对象。 */
  valueType: 'json';
  /** 是否满足当前消费节点的控制流可用性约束。 */
  available: boolean;
  /** 输出不可用时的控制流诊断。 */
  unavailableReason?: string;
}>;

/** 节点在统一值空间中公开的完整结果或单个 Published Output。 */
export type WorkflowNodeValueSymbol = WorkflowNodeResultSymbol | WorkflowNodeOutputSymbol;

/** 统一工作流值空间中的三类前端展示符号。 */
export type WorkflowSymbol =
  | WorkflowInputSymbol
  | WorkflowVariableSymbol
  | WorkflowNodeValueSymbol;

/** 从当前工作流快照派生的只读 Symbol Registry。 */
export type WorkflowSymbolRegistry = Readonly<{
  /** 工作流声明的运行输入。 */
  inputs: ReadonlyArray<WorkflowInputSymbol>;
  /** 工作流持久化的初始变量。 */
  variables: ReadonlyArray<WorkflowVariableSymbol>;
  /** 所有节点的 Published Outputs。 */
  nodeOutputs: ReadonlyArray<WorkflowNodeValueSymbol>;
}>;

/** 构建统一值目录所需的当前工作流编辑快照。 */
export type BuildWorkflowSymbolRegistryArgs = Readonly<{
  /** 工作流声明的运行输入。 */
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  /** 工作流变量的初始 JSON 对象。 */
  variables: JsonObject;
  /** 当前工作流画布节点。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 当前工作流画布连线。 */
  edges: ReadonlyArray<WorkflowCanvasEdge>;
  /** 当前 ValueExpr 所属消费节点；省略时列出全部节点输出。 */
  consumerNodeId?: string;
}>;

/**
 * 从工作流快照派生输入、变量和节点输出三类符号。
 *
 * 节点输出始终从 `getNodeValueOutputs` 读取，因此 native output、覆盖后的 native output
 * 和自定义 output binding 会保持与现有 Published Outputs 契约一致。
 */
export function buildWorkflowSymbolRegistry(
  args: BuildWorkflowSymbolRegistryArgs,
): WorkflowSymbolRegistry {
  const inputs = args.inputs.map((input): WorkflowInputSymbol => ({
    id: inputSymbolId(input.key),
    kind: 'workflow_input',
    name: input.key,
    label: input.key,
    valueType: 'text',
    available: true,
  }));

  const variables = Object.keys(args.variables).map((name): WorkflowVariableSymbol => ({
    id: variableSymbolId(name),
    kind: 'variable',
    name,
    label: name,
    valueType: 'json',
    available: true,
  }));

  const availability = buildWorkflowNodeOutputAvailabilityIndex(args);
  const nodeOutputs = args.nodes.flatMap((node): WorkflowNodeValueSymbol[] => {
    const outputs = getNodeValueOutputs(node.data);
    if (outputs.length === 0) return [];
    const nodeAvailability = availability.get(node.id)
      ?? { available: false, unavailableReason: '生产节点不存在' };
    return [
      createNodeResultSymbol(node, nodeAvailability),
      ...outputs.map((output) => createNodeOutputSymbol(
        node,
        output,
        nodeAvailability,
      )),
    ];
  });

  return { inputs, variables, nodeOutputs };
}

/** 把值选择器符号映射回现有 ValueExpr 持久化契约。 */
export function symbolToValueExpr(symbol: WorkflowSymbol): ValueExpr {
  switch (symbol.kind) {
    case 'workflow_input':
      return {
        type: 'ref',
        source: {
          type: 'workflow_input',
          key: symbol.name,
        },
        pointer: '',
      };
    case 'variable':
      return {
        type: 'ref',
        source: {
          type: 'variable',
          name: symbol.name,
        },
        pointer: '',
      };
    case 'node_output':
      return {
        type: 'ref',
        source: {
          type: 'node',
          node_id: symbol.nodeId,
        },
        pointer: `/${escapePointerToken(symbol.outputName)}`,
      };
    case 'node_result':
      return {
        type: 'ref',
        source: {
          type: 'node',
          node_id: symbol.nodeId,
        },
        pointer: '',
      };
  }
}

/** 创建节点完整 Published Outputs 对象的候选项。 */
function createNodeResultSymbol(
  node: WorkflowCanvasNode,
  availability: WorkflowSymbolAvailability,
): WorkflowNodeResultSymbol {
  const base = {
    id: nodeResultSymbolId(node.id),
    kind: 'node_result' as const,
    nodeId: node.id,
    outputName: null,
    name: node.id,
    nodeLabel: node.data.label,
    label: `${node.data.label} · 整个输出对象`,
    valueType: 'json' as const,
  };
  return availability.available
    ? { ...base, available: true }
    : {
        ...base,
        available: false,
        unavailableReason: availability.unavailableReason ?? '并非在所有执行路径上可用',
      };
}

/** 创建单个节点 Published Output 的展示符号。 */
function createNodeOutputSymbol(
  node: WorkflowCanvasNode,
  output: ValueOutputDescriptor,
  availability: WorkflowSymbolAvailability,
): WorkflowNodeOutputSymbol {
  const base = {
    id: nodeOutputSymbolId(node.id, output.name),
    kind: 'node_output' as const,
    nodeId: node.id,
    outputName: output.name,
    name: output.name,
    nodeLabel: node.data.label,
    label: `${node.data.label} · ${output.label}`,
    valueType: output.valueType,
  };

  return availability.available
    ? { ...base, available: true }
    : {
        ...base,
        available: false,
        unavailableReason: availability.unavailableReason ?? '并非在所有执行路径上可用',
      };
}

/** 创建带模板字面量约束的输入符号 ID。 */
function inputSymbolId(name: string): `input:${string}` {
  return `input:${encodeURIComponent(name)}`;
}

/** 创建带模板字面量约束的变量符号 ID。 */
function variableSymbolId(name: string): `variable:${string}` {
  return `variable:${encodeURIComponent(name)}`;
}

/** 创建不会与任意 Published Output 名称碰撞的完整结果 ID。 */
function nodeResultSymbolId(nodeId: string): `node:${string}:result` {
  return `node:${encodeURIComponent(nodeId)}:result`;
}

/** 创建带模板字面量约束的节点输出符号 ID。 */
function nodeOutputSymbolId(
  nodeId: string,
  outputName: string,
): `node:${string}:output:${string}` {
  return `node:${encodeURIComponent(nodeId)}:output:${encodeURIComponent(outputName)}`;
}

/** 按 RFC 6901 规则转义一个 JSON Pointer token。 */
function escapePointerToken(value: string): string {
  return value.replaceAll('~', '~0').replaceAll('/', '~1');
}
