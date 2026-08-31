import type {
  AutomationTarget,
  AqlQuery,
  UiExecutionPolicy,
  UiOperation,
  ValueExpr,
} from './contracts';
import type { KeyboardKey, KeyboardModifier } from './inputContracts';

/**
 * 以左上角“搜索”为语义锚点：最近的同名文字是侧栏联系人，第二近才是会话标题。
 * 只有侧栏联系人而右侧会话尚未切换完成时，显式的第二名不存在，查询不会误通过。
 */
const WECHAT_CONVERSATION_HEADER_QUERY =
  'nearest(anchor = text(name = "搜索"), target = text(name = $contact_name), direction = any, index = 2)';

/** 动作前选择最靠近窗口底边的同文本实例，即当前输入框中的待发送消息。 */
const WECHAT_PENDING_MESSAGE_QUERY =
  'nearest(anchor = viewport_edge(side = bottom), target = text(name = $message), direction = any, index = 1)';

/** 创建参数化 AQL v2 查询，供目标和后置条件共用同一契约。 */
function createWechatAqlQuery(
  source: string,
  bindings: AqlQuery['bindings'] = {},
): AqlQuery {
  return { language_version: 2, source, bindings };
}

/** 创建绑定微信 Application 节点的当前焦点输入目标。 */
export function createWechatInputTarget(applicationNodeId: string): AutomationTarget {
  return {
    scope: {
      type: 'application',
      resource: {
        producer_node_id: applicationNodeId,
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

/** 创建绑定微信 Application 节点的 AQL v2 视觉场景查询目标。 */
export function createWechatAqlTarget(
  applicationNodeId: string,
  source: string,
  bindings: AqlQuery['bindings'] = {},
): AutomationTarget {
  return {
    scope: {
      type: 'application',
      resource: {
        producer_node_id: applicationNodeId,
        output_name: 'session',
      },
    },
    locator: {
      type: 'query',
      query: createWechatAqlQuery(source, bindings),
    },
    backend_policy: {
      allow: ['ocr_small', 'send_input'],
      deny: [],
      prefer: ['ocr_small', 'send_input'],
    },
  };
}

/** 视觉读取与点击共用的有界目标等待。 */
export function createWechatVisualExecutionPolicy(): UiExecutionPolicy {
  return {
    target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 300 },
    postcondition_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 150 },
    postcondition: null,
  };
}

/** 焦点键盘输入不等待元素出现，只依赖已绑定窗口。 */
export function createWechatInputExecutionPolicy(): UiExecutionPolicy {
  return {
    target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    postcondition_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    postcondition: null,
  };
}

/** 点击联系人后只接受新鲜画面中右侧标题栏的唯一联系人名称。 */
export function createWechatOpenConversationExecutionPolicy(
  contactName: ValueExpr,
): UiExecutionPolicy {
  return {
    target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 300 },
    postcondition_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 150 },
    postcondition: {
      type: 'match_present',
      query: createWechatAqlQuery(WECHAT_CONVERSATION_HEADER_QUERY, {
        contact_name: contactName,
      }),
    },
  };
}

/** 发送消息要求会话标题保持原位，并确认输入框中的待发送文字实例已经消失。 */
export function createWechatSendMessageExecutionPolicy(
  contactName: ValueExpr,
): UiExecutionPolicy {
  return {
    target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    postcondition_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 150 },
    postcondition: {
      type: 'match_removed',
      query: createWechatAqlQuery(WECHAT_PENDING_MESSAGE_QUERY, {
        message: {
          type: 'ref',
          source: { type: 'workflow_input', key: 'message' },
          pointer: '',
        },
      }),
      stable_context: [createWechatAqlQuery(WECHAT_CONVERSATION_HEADER_QUERY, {
        contact_name: contactName,
      })],
    },
  };
}

/** 创建通过 Ctrl/Alt/Shift 组合键操作微信焦点的 UI 操作。 */
export function createWechatPressKeyOperation(
  applicationNodeId: string,
  key: KeyboardKey,
  modifiers: readonly KeyboardModifier[],
): Extract<UiOperation, { type: 'press_key' }> {
  return {
    type: 'press_key',
    target: createWechatInputTarget(applicationNodeId),
    chord: { key, modifiers: [...modifiers] },
  };
}

/** 创建从流程输入向微信当前焦点注入 Unicode 文本的 UI 操作。 */
export function createWechatTypeTextOperation(
  applicationNodeId: string,
  inputKey: 'contact_name' | 'message',
): Extract<UiOperation, { type: 'type_text' }> {
  return {
    type: 'type_text',
    target: createWechatInputTarget(applicationNodeId),
    value: {
      type: 'ref',
      source: { type: 'workflow_input', key: inputKey },
      pointer: '',
    },
  };
}

/** 创建从完整视觉场景执行 AQL 读取的 UI 操作。 */
export function createWechatAqlGetTextOperation(
  applicationNodeId: string,
  source: string,
  bindings: AqlQuery['bindings'] = {},
): Extract<UiOperation, { type: 'get_text' }> {
  return {
    type: 'get_text',
    target: createWechatAqlTarget(applicationNodeId, source, bindings),
  };
}

/** 创建通过 AQL 空间查询解析安全点再执行物理点击的 UI 操作。 */
export function createWechatAqlClickOperation(
  applicationNodeId: string,
  source: string,
  bindings: AqlQuery['bindings'] = {},
): Extract<UiOperation, { type: 'click' }> {
  return {
    type: 'click',
    target: createWechatAqlTarget(applicationNodeId, source, bindings),
  };
}
