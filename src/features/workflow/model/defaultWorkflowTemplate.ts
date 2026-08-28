import type {
  AutomationTarget,
  JsonObject,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 获取或启动微信并提供稳定 AppSession 的节点。 */
const WECHAT_APPLICATION_NODE_ID = 'wechat_application_1';

/** 打开微信全局搜索框的组合键节点。 */
const OPEN_SEARCH_NODE_ID = 'wechat_open_search_1';

/** 选中搜索框内已有文字的组合键节点。 */
const SELECT_SEARCH_TEXT_NODE_ID = 'wechat_select_search_text_1';

/** 输入目标群名称的节点。 */
const TYPE_GROUP_NAME_NODE_ID = 'wechat_type_group_name_1';

/** 打开当前首条搜索结果的节点。 */
const OPEN_GROUP_NODE_ID = 'wechat_open_group_1';

/** 输入测试消息的节点。 */
const TYPE_MESSAGE_NODE_ID = 'wechat_type_message_1';

/** 发送消息的节点。 */
const SEND_MESSAGE_NODE_ID = 'wechat_send_message_1';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '搜索微信群并发送测试消息';

/** 微信流程不依赖持久化变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 群名称和消息内容作为每次运行输入，避免写死在节点实现里。 */
export const DEFAULT_WORKFLOW_INPUTS = [
  { key: 'group_name', value_type: 'text' },
  { key: 'message', value_type: 'text' },
] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 当前验证场景的预填运行输入；用户可在运行前修改。 */
export const DEFAULT_RUN_INPUT_VALUES = {
  group_name: '三人行必有三狗',
  message: '测试消息',
} as const satisfies JsonObject;

/** AttachOrStart 可能启动微信，因此只声明应用启动能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  allow: ['process.application.launch'],
} as const satisfies WorkflowPermissions;

/** 默认选中应用节点，便于先核对 EXE 和窗口匹配条件。 */
export const DEFAULT_SELECTED_NODE_ID = WECHAT_APPLICATION_NODE_ID;

/**
 * 微信 4.x 不公开内部 UIA 元素，本流程使用 AppSession 锁定窗口，再向当前焦点发送键盘输入。
 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', outputBindings: {}, runState: 'idle' },
  },
  {
    id: WECHAT_APPLICATION_NODE_ID,
    kind: 'application',
    position: { x: 188, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.application },
    data: {
      kind: 'application',
      label: '打开微信',
      outputBindings: {},
      spec: {
        executable_path: 'C:\\Program Files\\Tencent\\Weixin\\Weixin.exe',
        arguments: [],
        window_title: { type: 'equal', value: '微信' },
        acquire_policy: 'attach_or_start',
        launch_timeout_ms: 15_000,
        cleanup_policy: 'leave_running',
        activation_policy: 'required',
      },
      runState: 'idle',
    },
  },
  createPressKeyNode(
    OPEN_SEARCH_NODE_ID,
    '打开搜索',
    { x: 402, y: 104 },
    { type: 'character', value: 'f' },
    ['control'],
  ),
  createDelayNode('wechat_wait_search_1', '等待搜索框', { x: 610, y: 104 }, 250),
  createPressKeyNode(
    SELECT_SEARCH_TEXT_NODE_ID,
    '选中搜索文字',
    { x: 788, y: 104 },
    { type: 'character', value: 'a' },
    ['control'],
  ),
  createTypeTextNode(
    TYPE_GROUP_NAME_NODE_ID,
    '输入群名称',
    { x: 1002, y: 104 },
    'group_name',
  ),
  createDelayNode('wechat_wait_results_1', '等待搜索结果', { x: 1002, y: 220 }, 600),
  createPressKeyNode(
    OPEN_GROUP_NODE_ID,
    '打开群聊',
    { x: 788, y: 220 },
    { type: 'enter' },
    [],
  ),
  createDelayNode('wechat_wait_chat_1', '等待群聊', { x: 610, y: 220 }, 500),
  createTypeTextNode(
    TYPE_MESSAGE_NODE_ID,
    '输入测试消息',
    { x: 402, y: 220 },
    'message',
  ),
  createPressKeyNode(
    SEND_MESSAGE_NODE_ID,
    '发送消息',
    { x: 188, y: 220 },
    { type: 'enter' },
    [],
  ),
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 28, y: 220 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', outputBindings: {}, runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板按两行蛇形布局串联搜索、打开群聊和发送消息。 */
export const DEFAULT_EDGES = [
  createDefaultEdge('edge_start_application', 'start_1', WECHAT_APPLICATION_NODE_ID),
  createDefaultEdge('edge_application_search', WECHAT_APPLICATION_NODE_ID, OPEN_SEARCH_NODE_ID),
  createDefaultEdge('edge_search_wait', OPEN_SEARCH_NODE_ID, 'wechat_wait_search_1'),
  createDefaultEdge('edge_wait_select', 'wechat_wait_search_1', SELECT_SEARCH_TEXT_NODE_ID),
  createDefaultEdge('edge_select_group_name', SELECT_SEARCH_TEXT_NODE_ID, TYPE_GROUP_NAME_NODE_ID),
  createDefaultEdge(
    'edge_group_name_wait',
    TYPE_GROUP_NAME_NODE_ID,
    'wechat_wait_results_1',
    'bottom',
    'top',
  ),
  createDefaultEdge('edge_wait_open_group', 'wechat_wait_results_1', OPEN_GROUP_NODE_ID, 'left'),
  createDefaultEdge('edge_open_group_wait', OPEN_GROUP_NODE_ID, 'wechat_wait_chat_1', 'left'),
  createDefaultEdge('edge_wait_message', 'wechat_wait_chat_1', TYPE_MESSAGE_NODE_ID, 'left'),
  createDefaultEdge('edge_message_send', TYPE_MESSAGE_NODE_ID, SEND_MESSAGE_NODE_ID, 'left'),
  createDefaultEdge('edge_send_end', SEND_MESSAGE_NODE_ID, 'end_1', 'left'),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

/** 创建只允许 SendInput、并绑定微信 AppSession 当前焦点的目标。 */
function createWechatInputTarget(): AutomationTarget {
  return {
    scope: {
      type: 'application',
      resource: {
        producer_node_id: WECHAT_APPLICATION_NODE_ID,
        output_name: 'session',
      },
    },
    locator: { type: 'focused' },
    backend_policy: {
      allow: ['send_input'],
      deny: [],
      prefer: ['send_input'],
    },
  };
}

/** 创建一次布局无关的组合键节点。 */
function createPressKeyNode(
  id: string,
  label: string,
  position: WorkflowCanvasNode['position'],
  key: Extract<
    Extract<WorkflowCanvasNode['data'], { kind: 'ui' }>['operation'],
    { type: 'press_key' }
  >['chord']['key'],
  modifiers: readonly ('control' | 'alt' | 'shift')[],
): WorkflowCanvasNode {
  return {
    id,
    kind: 'ui',
    position,
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label,
      outputBindings: {},
      operation: {
        type: 'press_key',
        target: createWechatInputTarget(),
        chord: { key, modifiers: [...modifiers] },
      },
      execution: {
        target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
      },
      runState: 'idle',
    },
  };
}

/** 创建从工作流输入读取 Unicode 文本的物理输入节点。 */
function createTypeTextNode(
  id: string,
  label: string,
  position: WorkflowCanvasNode['position'],
  inputKey: 'group_name' | 'message',
): WorkflowCanvasNode {
  return {
    id,
    kind: 'ui',
    position,
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label,
      outputBindings: {},
      operation: {
        type: 'type_text',
        target: createWechatInputTarget(),
        value: {
          type: 'ref',
          source: { type: 'workflow_input', key: inputKey },
          pointer: '',
        },
      },
      execution: {
        target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
      },
      runState: 'idle',
    },
  };
}

/** 创建用于吸收微信异步渲染时间的显式短等待。 */
function createDelayNode(
  id: string,
  label: string,
  position: WorkflowCanvasNode['position'],
  milliseconds: number,
): WorkflowCanvasNode {
  return {
    id,
    kind: 'delay',
    position,
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label,
      outputBindings: {},
      milliseconds,
      runState: 'idle',
    },
  };
}

/** 创建默认示例中的无分支连线。 */
function createDefaultEdge(
  id: string,
  sourceNodeId: string,
  targetNodeId: string,
  sourceSide: WorkflowCanvasEdge['source']['side'] = 'right',
  targetSide: WorkflowCanvasEdge['target']['side'] = 'left',
): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: sourceNodeId, side: sourceSide },
    target: { nodeId: targetNodeId, side: targetSide },
    data: { branch: null },
  };
}
