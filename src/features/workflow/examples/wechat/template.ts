import type {
  ControlPortId,
  JsonObject,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from '../../model/contracts';
import {
  WORKFLOW_NODE_SIZES,
  type EditableNodeKind,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
} from '../../model/workflowModel';
import {
  createWechatApplicationSpec,
  createWechatContactClickExecution,
  createWechatContactClickOperation,
  createWechatInputExecution,
  createWechatObservation,
  createWechatPressKeyOperation,
  createWechatTypeTextOperation,
  WECHAT_CONVERSATION_READY_QUERY,
  WECHAT_MESSAGE_SENT_QUERY,
  WECHAT_SEARCH_READY_QUERY,
  wechatInput,
} from './automation';

/** 默认工作区展示的完整微信自动化示例名称。 */
export const WECHAT_WORKFLOW_NAME = '微信：搜索联系人并发送消息';

/** 示例中的界面操作都依赖这个应用资源节点。 */
const WECHAT_APPLICATION_NODE_ID = 'open_wechat';

/** 示例不需要持久化可变状态。 */
export const WECHAT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 联系人和消息内容在每次运行前填写，不写入工作流定义。 */
export const WECHAT_WORKFLOW_INPUTS = [
  { key: '联系人', value_type: 'text' },
  { key: '消息内容', value_type: 'text' },
] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 打开微信需要显式授予启动桌面应用的能力。 */
export const WECHAT_WORKFLOW_PERMISSIONS = {
  allow: ['process.application.launch'],
} as const satisfies WorkflowPermissions;

/** 使用文件传输助手作为默认联系人，降低示例误发给真实联系人的风险。 */
export const WECHAT_RUN_INPUT_VALUES = {
  联系人: '文件传输助手',
  消息内容: 'ArgusFlow 测试消息',
} as const satisfies JsonObject;

/** 默认选中开始节点，让进入画布后的起点明确可见。 */
export const WECHAT_SELECTED_NODE_ID = 'start';

/**
 * 微信示例由普通画布节点组成，用户可以查看、修改或删除任意一步。
 *
 * 三组“重复检查”分别等待搜索页、联系人会话和发送结果；达到上限后进入
 * 对应的报错节点，不会无限等待。
 */
export const WECHAT_WORKFLOW_NODES = [
  workflowNode('start', 'start', 40, 100, {
    kind: 'start',
    label: '开始',
    outputBindings: {},
    runState: 'idle',
  }),
  workflowNode(WECHAT_APPLICATION_NODE_ID, 'application', 220, 100, {
    kind: 'application',
    label: '打开微信',
    outputBindings: {},
    spec: createWechatApplicationSpec(),
    runState: 'idle',
  }),
  workflowNode('open_search', 'ui', 430, 100, {
    kind: 'ui',
    label: '打开联系人搜索',
    outputBindings: {},
    operation: createWechatPressKeyOperation(
      WECHAT_APPLICATION_NODE_ID,
      { type: 'character', value: 'f' },
      ['control'],
    ),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('wait_for_search', 'loop', 640, 100, {
    kind: 'loop',
    label: '等待搜索页打开',
    outputBindings: {},
    maxIterations: 16,
    timeoutMs: 5_000,
    intervalMs: 300,
    runState: 'idle',
  }),
  workflowNode('check_search', 'observe', 840, 100, {
    kind: 'observe',
    label: '检查搜索页',
    outputBindings: {},
    observation: createWechatObservation(
      WECHAT_APPLICATION_NODE_ID,
      WECHAT_SEARCH_READY_QUERY,
      {},
    ),
    resultType: 'boolean',
    runState: 'idle',
  }),
  workflowNode('search_not_ready', 'fail', 640, 220, {
    kind: 'fail',
    label: '搜索页未打开',
    outputBindings: {},
    code: 'wechat_search_not_ready',
    message: {
      type: 'literal',
      value: '未能打开微信搜索。请确认微信窗口可以正常操作后重试。',
    },
    runState: 'idle',
  }),
  workflowNode('select_search_text', 'ui', 840, 320, {
    kind: 'ui',
    label: '清空搜索框',
    outputBindings: {},
    operation: createWechatPressKeyOperation(
      WECHAT_APPLICATION_NODE_ID,
      { type: 'character', value: 'a' },
      ['control'],
    ),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('type_contact', 'ui', 640, 320, {
    kind: 'ui',
    label: '输入联系人',
    outputBindings: {},
    operation: createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, '联系人'),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('select_contact', 'ui', 430, 320, {
    kind: 'ui',
    label: '选择联系人',
    outputBindings: {},
    operation: createWechatContactClickOperation(WECHAT_APPLICATION_NODE_ID),
    execution: createWechatContactClickExecution(),
    runState: 'idle',
  }),
  workflowNode('wait_for_conversation', 'loop', 220, 320, {
    kind: 'loop',
    label: '等待会话打开',
    outputBindings: {},
    maxIterations: 16,
    timeoutMs: 5_000,
    intervalMs: 300,
    runState: 'idle',
  }),
  workflowNode('check_conversation', 'observe', 20, 320, {
    kind: 'observe',
    label: '检查联系人会话',
    outputBindings: {},
    observation: createWechatObservation(
      WECHAT_APPLICATION_NODE_ID,
      WECHAT_CONVERSATION_READY_QUERY,
      { contact_name: wechatInput('联系人') },
    ),
    resultType: 'boolean',
    runState: 'idle',
  }),
  workflowNode('conversation_not_ready', 'fail', 220, 440, {
    kind: 'fail',
    label: '联系人会话未打开',
    outputBindings: {},
    code: 'wechat_conversation_not_ready',
    message: {
      type: 'literal',
      value: '未能打开联系人会话。请确认联系人名称后重试。',
    },
    runState: 'idle',
  }),
  workflowNode('type_message', 'ui', 20, 560, {
    kind: 'ui',
    label: '输入消息',
    outputBindings: {},
    operation: createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, '消息内容'),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('send_message', 'ui', 220, 560, {
    kind: 'ui',
    label: '发送消息',
    outputBindings: {},
    operation: createWechatPressKeyOperation(
      WECHAT_APPLICATION_NODE_ID,
      { type: 'enter' },
      [],
    ),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('wait_for_wechat_update', 'delay', 430, 560, {
    kind: 'delay',
    label: '等待微信更新',
    outputBindings: {},
    milliseconds: 800,
    runState: 'idle',
  }),
  workflowNode('wait_for_send_result', 'loop', 640, 560, {
    kind: 'loop',
    label: '等待发送结果',
    outputBindings: {},
    maxIterations: 16,
    timeoutMs: 5_000,
    intervalMs: 300,
    runState: 'idle',
  }),
  workflowNode('check_send_result', 'observe', 840, 560, {
    kind: 'observe',
    label: '检查发送结果',
    outputBindings: {},
    observation: createWechatObservation(
      WECHAT_APPLICATION_NODE_ID,
      WECHAT_MESSAGE_SENT_QUERY,
      {
        contact_name: wechatInput('联系人'),
        message: wechatInput('消息内容'),
      },
    ),
    resultType: 'boolean',
    runState: 'idle',
  }),
  workflowNode('send_result_unknown', 'fail', 640, 680, {
    kind: 'fail',
    label: '无法确认发送结果',
    outputBindings: {},
    code: 'wechat_send_result_unknown',
    message: {
      type: 'literal',
      value: '未能确认消息已发送。请打开微信检查当前会话后重试。',
    },
    runState: 'idle',
  }),
  workflowNode('end', 'end', 1_040, 560, {
    kind: 'end',
    label: '消息已发送',
    outputBindings: {},
    runState: 'idle',
  }),
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 示例连线完整表达正常路径、继续检查和达到上限后的失败路径。 */
export const WECHAT_WORKFLOW_EDGES = [
  workflowEdge('start', WECHAT_APPLICATION_NODE_ID),
  workflowEdge(WECHAT_APPLICATION_NODE_ID, 'open_search'),
  workflowEdge('open_search', 'wait_for_search'),
  workflowEdge('wait_for_search', 'check_search', 'iterate'),
  workflowEdge('wait_for_search', 'search_not_ready', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('check_search', 'select_search_text', 'true', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('check_search', 'wait_for_search', 'false', {
    sourceSide: 'top',
    targetSide: 'top',
  }),
  workflowEdge('check_search', 'wait_for_search', 'unknown', {
    sourceSide: 'bottom',
    targetSide: 'bottom',
  }),
  workflowEdge('select_search_text', 'type_contact', null, {
    sourceSide: 'left',
    targetSide: 'right',
  }),
  workflowEdge('type_contact', 'select_contact', null, {
    sourceSide: 'left',
    targetSide: 'right',
  }),
  workflowEdge('select_contact', 'wait_for_conversation', null, {
    sourceSide: 'left',
    targetSide: 'right',
  }),
  workflowEdge('wait_for_conversation', 'check_conversation', 'iterate', {
    sourceSide: 'left',
    targetSide: 'right',
  }),
  workflowEdge('wait_for_conversation', 'conversation_not_ready', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('check_conversation', 'type_message', 'true', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('check_conversation', 'wait_for_conversation', 'false', {
    sourceSide: 'top',
    targetSide: 'top',
  }),
  workflowEdge('check_conversation', 'wait_for_conversation', 'unknown', {
    sourceSide: 'bottom',
    targetSide: 'bottom',
  }),
  workflowEdge('type_message', 'send_message'),
  workflowEdge('send_message', 'wait_for_wechat_update'),
  workflowEdge('wait_for_wechat_update', 'wait_for_send_result'),
  workflowEdge('wait_for_send_result', 'check_send_result', 'iterate'),
  workflowEdge('wait_for_send_result', 'send_result_unknown', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('check_send_result', 'end', 'true'),
  workflowEdge('check_send_result', 'wait_for_send_result', 'false', {
    sourceSide: 'top',
    targetSide: 'top',
  }),
  workflowEdge('check_send_result', 'wait_for_send_result', 'unknown', {
    sourceSide: 'bottom',
    targetSide: 'bottom',
  }),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

/** 创建一个 kind 与 data 判别值严格一致的示例画布节点。 */
function workflowNode<Kind extends EditableNodeKind>(
  id: string,
  kind: Kind,
  x: number,
  y: number,
  data: Extract<WorkflowNodeData, { kind: Kind }>,
): WorkflowCanvasNode {
  return {
    id,
    kind,
    position: { x, y },
    size: { ...WORKFLOW_NODE_SIZES[kind] },
    data,
  };
}

type WorkflowEdgeSides = Readonly<{
  sourceSide?: WorkflowCanvasEdge['source']['side'];
  targetSide?: WorkflowCanvasEdge['target']['side'];
}>;

/** 创建带稳定分支名称和端口方向的示例画布连线。 */
function workflowEdge(
  source: string,
  target: string,
  branch: ControlPortId | null = null,
  sides: WorkflowEdgeSides = {},
): WorkflowCanvasEdge {
  return {
    id: `edge_${source}_${branch ?? 'next'}_${target}`,
    source: { nodeId: source, side: sides.sourceSide ?? 'right' },
    target: { nodeId: target, side: sides.targetSide ?? 'left' },
    data: { branch },
  };
}
