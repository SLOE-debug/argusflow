import type {
  AutomationTarget,
  UiExecutionPolicy,
  UiOperation,
  ValueExpr,
} from './contracts';
import type { KeyboardKey, KeyboardModifier } from './inputContracts';
import type { NormalizedRect } from './visual';

/** 搜索结果区域；按微信窗口比例限制 OCR，减少侧栏和标题重复命中。 */
export const WECHAT_SEARCH_RESULTS_REGION = {
  x: 0,
  y: 0,
  width: 0.58,
  height: 0.72,
} as const satisfies NormalizedRect;

/** 群聊标题区域；只在窗口上半部寻找群名称。 */
export const WECHAT_HEADER_REGION = {
  x: 0.38,
  y: 0,
  width: 0.62,
  height: 0.22,
} as const satisfies NormalizedRect;

/** 消息区域；验证文字只在聊天内容和输入区附近寻找。 */
export const WECHAT_MESSAGE_REGION = {
  x: 0.34,
  y: 0.28,
  width: 0.66,
  height: 0.72,
} as const satisfies NormalizedRect;

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

/** 创建绑定微信 Application 节点的动态视觉文字目标。 */
export function createWechatVisualTarget(
  applicationNodeId: string,
  text: ValueExpr,
  exact: boolean,
  region: NormalizedRect,
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
      type: 'visual',
      query: { text, exact, region },
    },
    backend_policy: {
      allow: [],
      deny: [],
      prefer: [],
    },
  };
}

/** 视觉读取与点击共用的有界目标等待。 */
export function createWechatVisualExecutionPolicy(): UiExecutionPolicy {
  return {
    target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 300 },
  };
}

/** 焦点键盘输入不等待元素出现，只依赖已绑定窗口。 */
export function createWechatInputExecutionPolicy(): UiExecutionPolicy {
  return {
    target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
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
  inputKey: 'group_name' | 'message',
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

/** 创建从稳定画面读取一个文字事实的 UI 操作。 */
export function createWechatVisualGetTextOperation(
  applicationNodeId: string,
  text: ValueExpr,
  exact: boolean,
  region: NormalizedRect,
): Extract<UiOperation, { type: 'get_text' }> {
  return {
    type: 'get_text',
    target: createWechatVisualTarget(applicationNodeId, text, exact, region),
  };
}

/** 创建通过视觉解析安全点再执行物理点击的 UI 操作。 */
export function createWechatVisualClickOperation(
  applicationNodeId: string,
  text: ValueExpr,
  exact: boolean,
  region: NormalizedRect,
): Extract<UiOperation, { type: 'click' }> {
  return {
    type: 'click',
    target: createWechatVisualTarget(applicationNodeId, text, exact, region),
  };
}
