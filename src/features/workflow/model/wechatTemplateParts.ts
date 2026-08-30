import type {
  AutomationTarget,
  AqlQuery,
  UiExecutionPolicy,
  UiOperation,
  ValueExpr,
} from './contracts';
import type { KeyboardKey, KeyboardModifier } from './inputContracts';

/** 微信主窗口右侧会话标题栏，排除左侧联系人列表中的同名文字。 */
const WECHAT_CONVERSATION_HEADER_REGION = {
  x: 0.34,
  y: 0,
  width: 0.66,
  height: 0.13,
} as const;

/** 微信主窗口右侧消息区，排除标题栏、联系人列表和底部输入框。 */
const WECHAT_CONVERSATION_MESSAGES_REGION = {
  x: 0.34,
  y: 0.13,
  width: 0.66,
  height: 0.64,
} as const;

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
      query: { language_version: 2, source, bindings },
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
      type: 'text_present',
      query: {
        text: contactName,
        exact: true,
        region: WECHAT_CONVERSATION_HEADER_REGION,
      },
    },
  };
}

/** 发送消息要求同一会话保持不变，且消息区中的同文案实例数量增加。 */
export function createWechatSendMessageExecutionPolicy(
  contactName: ValueExpr,
): UiExecutionPolicy {
  return {
    target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    postcondition_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 150 },
    postcondition: {
      type: 'new_text',
      query: {
        text: {
          type: 'ref',
          source: { type: 'workflow_input', key: 'message' },
          pointer: '',
        },
        exact: true,
        region: WECHAT_CONVERSATION_MESSAGES_REGION,
      },
      stable_context: [{
        text: contactName,
        exact: true,
        region: WECHAT_CONVERSATION_HEADER_REGION,
      }],
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
