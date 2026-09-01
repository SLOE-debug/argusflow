import type {
  JsonValue,
  ValueOutputDescriptor,
  WorkflowNodeContract,
} from './contracts';
import { createDefaultApplicationSpec } from '../nodes/workflowApplication';
import { createDefaultBrowserSpec } from '../nodes/workflowBrowser';
import { createDefaultCommandOperation } from '../nodes/workflowCommand';
import {
  createDefaultUiExecutionPolicy,
  createDefaultUiOperation,
} from '../nodes/workflowAction';
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
  /** 返回随节点业务配置变化的已知原生值输出。 */
  outputs?: (data: NodeDataOf<Kind>) => ReadonlyArray<ValueOutputDescriptor>;
}>;

/** workflowModel 可以按 kind 统一调用的类型擦除 codec 边界。 */
export type WorkflowNodeDefinitionCodec = Readonly<{
  /** 编辑器和注册表共享的节点判别键。 */
  kind: EditableNodeKind;
  typeId: string;
  version: number;
  create: () => WorkflowNodeData;
  encode: (data: WorkflowNodeData) => JsonValue;
  outputs: (data: WorkflowNodeData) => ReadonlyArray<ValueOutputDescriptor>;
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
    outputs: (data) => spec.outputs?.(data as NodeDataOf<Kind>) ?? [],
  };
}

/** 编辑器节点的 definition/default/payload 单一注册源。 */
export const WORKFLOW_NODE_DEFINITIONS = {
  start: defineNode('start', {
    typeId: 'argus.start',
    version: 1,
    create: () => ({
      kind: 'start',
      label: '开始',
      outputBindings: {},
      runState: 'idle',
    }),
    encode: () => ({}),
  }),
  log: defineNode('log', {
    typeId: 'argus.log',
    version: 1,
    create: () => ({
      kind: 'log',
      label: '记录日志',
      outputBindings: {},
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
      label: '查看结果',
      outputBindings: {},
      value: { type: 'literal', value: '' },
      runState: 'idle',
    }),
    encode: (data) => ({ value: data.value }),
  }),
  delay: defineNode('delay', {
    typeId: 'argus.delay',
    version: 1,
    create: () => ({
      kind: 'delay',
      label: '等待一段时间',
      outputBindings: {},
      milliseconds: 500,
      runState: 'idle',
    }),
    encode: (data) => ({ milliseconds: data.milliseconds }),
  }),
  condition: defineNode('condition', {
    typeId: 'argus.condition',
    version: 1,
    create: () => ({
      kind: 'condition',
      label: '条件判断',
      outputBindings: {},
      left: {
        type: 'ref',
        source: { type: 'variable', name: 'enabled' },
        pointer: '',
      },
      operator: 'equal',
      right: { type: 'literal', value: true },
      runState: 'idle',
    }),
    encode: (data) => ({
      left: data.left,
      operator: data.operator,
      right: isUnaryCondition(data.operator) ? null : data.right,
    }),
  }),
  observe: defineNode('observe', {
    typeId: 'argus.observe',
    version: 1,
    create: () => ({
      kind: 'observe',
      label: '检查界面',
      outputBindings: {},
      resultType: 'boolean',
      observation: {
        scope: { type: 'current' },
        query: {
          language_version: 3,
          source: 'exists(button())',
          bindings: {},
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
        policy: { mode: 'once' },
      },
      runState: 'idle',
    }),
    encode: (data) => ({ observation: data.observation }),
    outputs: () => [{ name: 'result', valueType: 'json', label: '检查结果' }],
  }),
  loop: defineNode('loop', {
    typeId: 'argus.loop',
    version: 2,
    create: () => ({
      kind: 'loop',
      label: '重复执行',
      outputBindings: {},
      bodyScopeId: `scope-${crypto.randomUUID()}`,
      maxIterations: 10,
      timeoutMs: 30_000,
      intervalMs: 500,
      runState: 'idle',
    }),
    encode: (data) => ({
      body_scope_id: data.bodyScopeId,
      max_iterations: data.maxIterations,
      timeout_ms: data.timeoutMs,
      interval_ms: data.intervalMs,
    }),
  }),
  loopEntry: defineNode('loopEntry', {
    typeId: 'argus.loop.entry',
    version: 1,
    create: () => ({
      kind: 'loopEntry', label: '每轮开始', outputBindings: {}, runState: 'idle',
    }),
    encode: () => ({}),
  }),
  loopContinue: defineNode('loopContinue', {
    typeId: 'argus.loop.continue',
    version: 1,
    create: () => ({
      kind: 'loopContinue', label: '继续下一轮', outputBindings: {}, runState: 'idle',
    }),
    encode: () => ({}),
  }),
  loopComplete: defineNode('loopComplete', {
    typeId: 'argus.loop.complete',
    version: 1,
    create: () => ({
      kind: 'loopComplete', label: '完成循环', outputBindings: {}, runState: 'idle',
    }),
    encode: () => ({}),
  }),
  variable: defineNode('variable', {
    typeId: 'argus.variable.set',
    version: 1,
    create: () => ({
      kind: 'variable',
      label: '设置变量',
      outputBindings: {},
      assignments: [{ name: '', value: { type: 'literal', value: null } }],
      runState: 'idle',
    }),
    encode: (data) => ({ assignments: data.assignments }),
  }),
  application: defineNode('application', {
    typeId: 'argus.application',
    version: 1,
    create: () => ({
      kind: 'application',
      label: '打开应用',
      outputBindings: {},
      spec: createDefaultApplicationSpec(),
      runState: 'idle',
    }),
    encode: (data) => ({ spec: data.spec }),
  }),
  browser: defineNode('browser', {
    typeId: 'argus.browser',
    version: 2,
    create: () => ({
      kind: 'browser',
      label: '打开浏览器',
      outputBindings: {},
      spec: createDefaultBrowserSpec(),
      runState: 'idle',
    }),
    encode: (data) => ({ spec: data.spec }),
  }),
  navigate: defineNode('navigate', {
    typeId: 'argus.browser.operation',
    version: 1,
    create: () => ({
      kind: 'navigate',
      label: '打开网页',
      outputBindings: {},
      operation: {
        type: 'navigate',
        browser: { producer_node_id: '', output_name: 'session' },
        url: { type: 'literal', value: '' },
      },
      runState: 'idle',
    }),
    encode: (data) => ({ operation: data.operation }),
  }),
  ui: defineNode('ui', {
    typeId: 'argus.ui',
    version: 5,
    create: () => ({
      kind: 'ui',
      label: '操作界面',
      outputBindings: {},
      operation: createDefaultUiOperation(),
      execution: createDefaultUiExecutionPolicy(),
      runState: 'idle',
    }),
    encode: (data) => ({
      operation: data.operation,
      execution: data.execution,
    }),
    outputs: () => [],
  }),
  command: defineNode('command', {
    typeId: 'argus.command',
    version: 1,
    create: () => ({
      kind: 'command',
      label: '执行命令',
      outputBindings: {},
      operation: createDefaultCommandOperation(),
      runState: 'idle',
    }),
    encode: (data) => ({ operation: data.operation }),
    outputs: () => [
      { name: 'stdout', valueType: 'text', label: '标准输出' },
      { name: 'stderr', valueType: 'text', label: '错误输出' },
      { name: 'exit_code', valueType: 'json', label: '退出码' },
    ],
  }),
  format: defineNode('format', {
    typeId: 'argus.data.format',
    version: 1,
    create: () => ({
      kind: 'format',
      label: '整理文本',
      outputBindings: {},
      operation: {
        items: { type: 'literal', value: [] },
        fields: ['title', 'url'],
        column_separator: '\t',
        row_separator: '\r\n',
        include_header: false,
      },
      runState: 'idle',
    }),
    encode: (data) => ({ operation: data.operation }),
    outputs: () => [{ name: 'text', valueType: 'text', label: '整理后的文本' }],
  }),
  component: defineNode('component', {
    typeId: 'argus.component',
    version: 2,
    create: () => {
      return {
        kind: 'component',
        label: '组合步骤',
        outputBindings: {},
        component: {
          component_id: '00000000-0000-0000-0000-000000000000',
          component_version: '0.0.0',
          inputs: {},
        },
        componentName: '尚未选择',
        componentOutputs: [],
        componentDefinition: {
          schema_version: 2,
          id: '00000000-0000-0000-0000-000000000000',
          version: '0.0.0',
          name: '尚未选择',
          inputs: [],
          outputs: [],
          graph: { root_scope_id: '', scopes: [] },
        },
        runState: 'idle',
      };
    },
    encode: (data) => data.component,
    outputs: (data) => data.componentOutputs.map((output) => ({
      name: output.name,
      valueType: 'json',
      label: output.name,
    })),
  }),
  fail: defineNode('fail', {
    typeId: 'argus.fail',
    version: 1,
    create: () => ({
      kind: 'fail',
      label: '停止并报错',
      outputBindings: {},
      code: 'workflow_failed',
      message: { type: 'literal', value: '流程没有达到预期结果。请检查前面的步骤后重试。' },
      runState: 'idle',
    }),
    encode: (data) => ({ code: data.code, message: data.message }),
  }),
  end: defineNode('end', {
    typeId: 'argus.end',
    version: 1,
    create: () => ({
      kind: 'end',
      label: '结束',
      outputBindings: {},
      runState: 'idle',
    }),
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

/** 返回节点原生输出与用户自定义输出组成的可枚举 Published Outputs。 */
export function getNodeValueOutputs(
  data: WorkflowNodeData,
): ReadonlyArray<ValueOutputDescriptor> {
  const nativeOutputs = getNativeNodeValueOutputs(data);
  const customNames = new Set(Object.keys(data.outputBindings));
  const visibleNativeOutputs = nativeOutputs.map((output) => customNames.has(output.name)
    ? {
        ...output,
        valueType: 'json' as const,
        label: `${output.label}（已覆盖）`,
      }
    : output);
  const nativeNames = new Set(nativeOutputs.map((output) => output.name));
  const additionalCustomOutputs = [...customNames]
    .filter((name) => !nativeNames.has(name))
    .map((name) => ({
      name,
      valueType: 'json' as const,
      label: `${name}（自定义）`,
    }));
  return [...visibleNativeOutputs, ...additionalCustomOutputs];
}

/** 返回注册节点自身公开、尚未叠加用户映射的输出描述。 */
export function getNativeNodeValueOutputs(
  data: WorkflowNodeData,
): ReadonlyArray<ValueOutputDescriptor> {
  return WORKFLOW_NODE_DEFINITIONS[data.kind].outputs(data);
}

/** Condition 一元运算符不保存右表达式。 */
export const isUnaryCondition = (operator?: string): boolean => (
  operator === 'exists'
  || operator === 'not_exists'
  || operator === 'is_empty'
  || operator === 'not_empty'
);
