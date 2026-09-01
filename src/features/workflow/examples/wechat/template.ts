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
import type { WorkflowDocuments } from '../../studio/workflowScopes';
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

/** 三个 While 容器各自拥有的独立子作用域。 */
const SEARCH_SCOPE_ID = 'scope_wait_for_search';
const CONVERSATION_SCOPE_ID = 'scope_wait_for_conversation';
const SEND_SCOPE_ID = 'scope_wait_for_send_result';

/**
 * 微信示例由普通画布节点组成，用户可以查看、修改或删除任意一步。
 *
 * 三组“重复检查”分别等待搜索页、联系人会话和发送结果；达到上限后进入
 * 对应的报错节点，不会无限等待。
 */
const WECHAT_WORKFLOW_ALL_NODES = [
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
    bodyScopeId: SEARCH_SCOPE_ID,
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
  workflowNode('search_not_ready', 'fail', 779, 460, {
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
  workflowNode('select_search_text', 'ui', 1_160, 100, {
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
  workflowNode('type_contact', 'ui', 1_370, 100, {
    kind: 'ui',
    label: '输入联系人',
    outputBindings: {},
    operation: createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, '联系人'),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('select_contact', 'ui', 1_580, 100, {
    kind: 'ui',
    label: '选择联系人',
    outputBindings: {},
    operation: createWechatContactClickOperation(WECHAT_APPLICATION_NODE_ID),
    execution: createWechatContactClickExecution(),
    runState: 'idle',
  }),
  workflowNode('wait_for_conversation', 'loop', 1_790, 100, {
    kind: 'loop',
    label: '等待会话打开',
    outputBindings: {},
    bodyScopeId: CONVERSATION_SCOPE_ID,
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
  workflowNode('conversation_not_ready', 'fail', 1_929, 460, {
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
  workflowNode('type_message', 'ui', 2_310, 100, {
    kind: 'ui',
    label: '输入消息',
    outputBindings: {},
    operation: createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, '消息内容'),
    execution: createWechatInputExecution(),
    runState: 'idle',
  }),
  workflowNode('send_message', 'ui', 2_520, 100, {
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
  workflowNode('wait_for_wechat_update', 'delay', 2_730, 100, {
    kind: 'delay',
    label: '等待微信更新',
    outputBindings: {},
    milliseconds: 800,
    runState: 'idle',
  }),
  workflowNode('wait_for_send_result', 'loop', 2_940, 100, {
    kind: 'loop',
    label: '等待发送结果',
    outputBindings: {},
    bodyScopeId: SEND_SCOPE_ID,
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
  workflowNode('send_result_unknown', 'fail', 3_079, 460, {
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
  workflowNode('end', 'end', 3_460, 100, {
    kind: 'end',
    label: '消息已发送',
    outputBindings: {},
    runState: 'idle',
  }),
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 根画布只保留 While 容器；三项检查移动到各自子作用域。 */
export const WECHAT_WORKFLOW_NODES = WECHAT_WORKFLOW_ALL_NODES.filter((node) => ![
  'check_search',
  'check_conversation',
  'check_send_result',
].includes(node.id));

/** 示例连线完整表达正常路径、继续检查和达到上限后的失败路径。 */
export const WECHAT_WORKFLOW_EDGES = [
  workflowEdge('start', WECHAT_APPLICATION_NODE_ID),
  workflowEdge(WECHAT_APPLICATION_NODE_ID, 'open_search'),
  workflowEdge('open_search', 'wait_for_search'),
  workflowEdge('wait_for_search', 'select_search_text', 'completed'),
  workflowEdge('wait_for_search', 'search_not_ready', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('select_search_text', 'type_contact'),
  workflowEdge('type_contact', 'select_contact'),
  workflowEdge('select_contact', 'wait_for_conversation'),
  workflowEdge('wait_for_conversation', 'type_message', 'completed'),
  workflowEdge('wait_for_conversation', 'conversation_not_ready', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
  workflowEdge('type_message', 'send_message'),
  workflowEdge('send_message', 'wait_for_wechat_update'),
  workflowEdge('wait_for_wechat_update', 'wait_for_send_result'),
  workflowEdge('wait_for_send_result', 'end', 'completed'),
  workflowEdge('wait_for_send_result', 'send_result_unknown', 'exhausted', {
    sourceSide: 'bottom',
    targetSide: 'top',
  }),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

/** 默认工作流根作用域 ID。 */
export const WECHAT_ROOT_SCOPE_ID = 'workflow_root';

/** 三个 While 子图都用 Entry → Observe → Continue/Complete 的显式 DAG。 */
const SEARCH_SCOPE = loopScope(
  SEARCH_SCOPE_ID,
  'wait_for_search',
  requiredNode('check_search'),
);
const CONVERSATION_SCOPE = loopScope(
  CONVERSATION_SCOPE_ID,
  'wait_for_conversation',
  requiredNode('check_conversation'),
);
const SEND_SCOPE = loopScope(
  SEND_SCOPE_ID,
  'wait_for_send_result',
  requiredNode('check_send_result'),
);

/** 默认示例交给多文档 Store 的完整作用域表。 */
export const WECHAT_WORKFLOW_DOCUMENTS = {
  [WECHAT_ROOT_SCOPE_ID]: {
    nodes: WECHAT_WORKFLOW_NODES,
    edges: WECHAT_WORKFLOW_EDGES,
  },
  [SEARCH_SCOPE_ID]: SEARCH_SCOPE.document,
  [CONVERSATION_SCOPE_ID]: CONVERSATION_SCOPE.document,
  [SEND_SCOPE_ID]: SEND_SCOPE.document,
} as const satisfies WorkflowDocuments;

/** 默认示例全部作用域的父容器和固定边界。 */
export const WECHAT_SCOPE_METADATA = {
  [WECHAT_ROOT_SCOPE_ID]: {
    parent: null,
    boundary: { type: 'workflow', entry_node_id: 'start' },
  },
  [SEARCH_SCOPE_ID]: {
    parent: { scope_id: WECHAT_ROOT_SCOPE_ID, node_id: 'wait_for_search' },
    boundary: SEARCH_SCOPE.boundary,
  },
  [CONVERSATION_SCOPE_ID]: {
    parent: { scope_id: WECHAT_ROOT_SCOPE_ID, node_id: 'wait_for_conversation' },
    boundary: CONVERSATION_SCOPE.boundary,
  },
  [SEND_SCOPE_ID]: {
    parent: { scope_id: WECHAT_ROOT_SCOPE_ID, node_id: 'wait_for_send_result' },
    boundary: SEND_SCOPE.boundary,
  },
} as const satisfies import('../../model/workflowModel').WorkflowScopeMetadataMap;

/** 创建一个带固定边界的 While 子图；观察结果只在本作用域内分支。 */
function loopScope(
  scopeId: string,
  _ownerNodeId: string,
  observation: WorkflowCanvasNode,
) {
  const entry = workflowNode(`${scopeId}_entry`, 'loopEntry', 40, 120, {
    kind: 'loopEntry',
    label: '每轮开始',
    outputBindings: {},
    runState: 'idle',
  });
  const continueNode = workflowNode(`${scopeId}_continue`, 'loopContinue', 500, 190, {
    kind: 'loopContinue',
    label: '继续下一轮',
    outputBindings: {},
    runState: 'idle',
  });
  const complete = workflowNode(`${scopeId}_complete`, 'loopComplete', 500, 50, {
    kind: 'loopComplete',
    label: '条件成立，完成循环',
    outputBindings: {},
    runState: 'idle',
  });
  /** 示例观察节点移动到局部坐标，但保留强类型业务配置与稳定 ID。 */
  const check = { ...observation, position: { x: 250, y: 120 } };
  return {
    document: {
      nodes: [entry, check, continueNode, complete],
      edges: [
        workflowEdge(entry.id, check.id),
        workflowEdge(check.id, complete.id, 'true'),
        workflowEdge(check.id, continueNode.id, 'false'),
        workflowEdge(check.id, continueNode.id, 'unknown'),
      ],
    },
    boundary: {
      type: 'loop' as const,
      entry_node_id: entry.id,
      continue_node_id: continueNode.id,
      complete_node_id: complete.id,
    },
  };
}

/** 从完整示例清单中取得必需节点，并在源码变更时尽早失败。 */
function requiredNode(nodeId: string): WorkflowCanvasNode {
  const node = WECHAT_WORKFLOW_ALL_NODES.find((candidate) => candidate.id === nodeId);
  if (!node) throw new Error(`微信示例缺少节点 '${nodeId}'`);
  return node;
}

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
