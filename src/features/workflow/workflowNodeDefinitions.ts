import type { JsonValue, WorkflowNodeContract } from './contracts';
import { createDefaultApplicationSpec } from './workflowApplication';
import { createDefaultBrowserSpec } from './workflowBrowser';
import { createDefaultCommandOperation } from './workflowCommand';
import { createDefaultUiOperation } from './workflowAction';
import type { EditableNodeKind, WorkflowNodeData } from './workflowModel';

type NodeDataOf<Kind extends EditableNodeKind> = Extract<
  WorkflowNodeData,
  { kind: Kind }
>;

type NodeDefinitionSpec<Kind extends EditableNodeKind> = Readonly<{
  /** 对应 Rust NodeCompiler 的稳定类型 ID。 */
  typeId: string;
  /** 当前 payload 契约版本。 */
  version: number;
  /** 创建字段完整且立即可编辑的节点状态。 */
  create: () => NodeDataOf<Kind>;
  /** 只编码对应节点类型的业务 payload。 */
  encode: (data: NodeDataOf<Kind>) => JsonValue;
}>;

/** workflowModel 可以按 kind 统一调用的类型擦除 codec 边界。 */
export type WorkflowNodeDefinitionCodec = Readonly<{
  /** 编辑器和注册表共享的节点判别键。 */
  kind: EditableNodeKind;
  typeId: string;
  version: number;
  create: () => WorkflowNodeData;
  encode: (data: WorkflowNodeData) => JsonValue;
}>;

/**
 * 定义一个节点 codec，并把唯一的判别联合断言封装在注册边界。
 *
 * 调用方始终以 `data.kind` 查找同键 codec，因此 encode 收到的数据类型由 registry key
 * 保证；具体 codec 内仍保持完整的 Extract 类型，不把 any 泄露给业务模块。
 */
function defineNode<Kind extends EditableNodeKind>(
  kind: Kind,
  spec: NodeDefinitionSpec<Kind>,
): WorkflowNodeDefinitionCodec {
  return {
    kind,
    typeId: spec.typeId,
    version: spec.version,
    create: spec.create,
    encode: (data) => spec.encode(data as NodeDataOf<Kind>),
  };
}

/** 编辑器节点的 definition/default/payload 单一注册源。 */
export const WORKFLOW_NODE_DEFINITIONS = {
  start: defineNode('start', {
    typeId: 'argus.start',
    version: 1,
    create: () => ({ kind: 'start', label: '开始', runState: 'idle' }),
    encode: () => ({}),
  }),
  log: defineNode('log', {
    typeId: 'argus.log',
    version: 1,
    create: () => ({
      kind: 'log',
      label: '日志',
      message: '记录一条运行信息',
      runState: 'idle',
    }),
    encode: (data) => ({ message: data.message }),
  }),
  debug: defineNode('debug', {
    typeId: 'argus.debug',
    version: 1,
    create: () => ({
      kind: 'debug',
      label: '调试输出',
      value: { type: 'literal', value: '' },
      runState: 'idle',
    }),
    encode: (data) => ({ value: data.value }),
  }),
  delay: defineNode('delay', {
    typeId: 'argus.delay',
    version: 1,
    create: () => ({ kind: 'delay', label: '等待', milliseconds: 500, runState: 'idle' }),
    encode: (data) => ({ milliseconds: data.milliseconds }),
  }),
  condition: defineNode('condition', {
    typeId: 'argus.condition',
    version: 1,
    create: () => ({
      kind: 'condition',
      label: '条件',
      pointer: '/enabled',
      operator: 'equal',
      operand: true,
      runState: 'idle',
    }),
    encode: (data) => ({
      predicate: {
        pointer: data.pointer,
        operator: data.operator,
        operand: isUnaryCondition(data.operator) ? null : data.operand,
      },
    }),
  }),
  application: defineNode('application', {
    typeId: 'argus.application',
    version: 1,
    create: () => ({
      kind: 'application',
      label: '打开或连接应用',
      spec: createDefaultApplicationSpec(),
      runState: 'idle',
    }),
    encode: (data) => ({ spec: data.spec }),
  }),
  browser: defineNode('browser', {
    typeId: 'argus.browser',
    version: 1,
    create: () => ({
      kind: 'browser',
      label: '打开浏览器',
      spec: createDefaultBrowserSpec(),
      runState: 'idle',
    }),
    encode: (data) => ({ spec: data.spec }),
  }),
  ui: defineNode('ui', {
    typeId: 'argus.ui',
    version: 1,
    create: () => ({
      kind: 'ui',
      label: '界面操作',
      operation: createDefaultUiOperation(),
      runState: 'idle',
    }),
    encode: (data) => ({ operation: data.operation }),
  }),
  command: defineNode('command', {
    typeId: 'argus.command',
    version: 1,
    create: () => ({
      kind: 'command',
      label: '执行命令',
      operation: createDefaultCommandOperation(),
      runState: 'idle',
    }),
    encode: (data) => ({ operation: data.operation }),
  }),
  end: defineNode('end', {
    typeId: 'argus.end',
    version: 1,
    create: () => ({ kind: 'end', label: '结束', runState: 'idle' }),
    encode: () => ({}),
  }),
} satisfies Readonly<Record<EditableNodeKind, WorkflowNodeDefinitionCodec>>;

/** 将编辑器节点编码为后端 registry 驱动的 definition envelope。 */
export function encodeNodeDefinition(
  data: WorkflowNodeData,
): Pick<WorkflowNodeContract, 'type_id' | 'version' | 'payload'> {
  const codec = WORKFLOW_NODE_DEFINITIONS[data.kind];
  return {
    type_id: codec.typeId,
    version: codec.version,
    payload: codec.encode(data),
  };
}

/** 创建指定注册节点的默认编辑器数据。 */
export function createRegisteredNodeData(kind: EditableNodeKind): WorkflowNodeData {
  return WORKFLOW_NODE_DEFINITIONS[kind].create();
}

/** Condition 一元运算符不保存 operand。 */
export const isUnaryCondition = (operator?: string): boolean => (
  operator === 'exists'
  || operator === 'not_exists'
  || operator === 'is_empty'
  || operator === 'not_empty'
);
