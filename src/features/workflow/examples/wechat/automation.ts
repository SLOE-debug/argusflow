import type {
  ApplicationSpec,
  AqlQuery,
  AutomationTarget,
  ObserveSpec,
  UiExecutionPolicy,
  UiOperation,
  ValueExpr,
} from '../../model/contracts';
import type {
  KeyboardKey,
  KeyboardModifier,
} from '../../model/inputContracts';

/** 微信示例公开给使用者填写的两个输入。 */
export type WechatWorkflowInputKey = '联系人' | '消息内容';

/** 搜索结果以“最常使用”为锚点，减少同名文字造成的误点。 */
export const WECHAT_CONTACT_RESULT_SELECTOR =
  'nearest(anchor = text(name = "最常使用"), target = text(name = $contact_name), direction = below, index = 1)';

/** 右侧会话标题是联系人名称相对搜索入口的第二个同名文字。 */
const WECHAT_CONVERSATION_HEADER_SELECTOR =
  'nearest(anchor = text(name = "搜索"), target = text(name = $contact_name), direction = any, index = 2)';

/** 消息发送后应出现在当前会话底部附近。 */
const WECHAT_LATEST_MESSAGE_SELECTOR =
  'nearest(anchor = viewport_edge(side = bottom), target = text(name = $message), direction = any, index = 1)';

/** 检查联系人会话是否已经打开。 */
export const WECHAT_CONVERSATION_READY_QUERY =
  `exists(${WECHAT_CONVERSATION_HEADER_SELECTOR})`;

/** 微信搜索页出现网络结果区域后，才向搜索框写入联系人名称。 */
export const WECHAT_SEARCH_READY_QUERY = 'exists(text(name contains "网络结果"))';

/**
 * 检查消息是否已出现在当前会话，并排除微信显示“重新发送”的失败状态。
 *
 * 这是示例工作流声明的成功条件；通用运行时只负责执行 AQL 查询。
 */
export const WECHAT_MESSAGE_SENT_QUERY = [
  'all_of(',
  `  exists(${WECHAT_CONVERSATION_HEADER_SELECTOR}),`,
  `  exists(${WECHAT_LATEST_MESSAGE_SELECTOR}),`,
  '  not(exists(text(name contains "重新发送")))',
  ')',
].join('\n');

/** 微信桌面应用的获取方式和窗口识别规则。 */
export function createWechatApplicationSpec(): ApplicationSpec {
  return {
    executable_path: 'C:\\Program Files\\Tencent\\Weixin\\Weixin.exe',
    arguments: [],
    window_title: { type: 'equal', value: '微信' },
    acquire_policy: 'attach_or_start',
    launch_timeout_ms: 15_000,
    cleanup_policy: 'leave_running',
    activation_policy: 'required',
  };
}

/** 从工作流输入读取用户每次运行时填写的文本。 */
export function wechatInput(key: WechatWorkflowInputKey): ValueExpr {
  return {
    type: 'ref',
    source: { type: 'workflow_input', key },
    pointer: '',
  };
}

/** 创建在微信窗口当前焦点执行的组合键动作。 */
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

/** 创建向微信当前输入位置键入工作流输入的动作。 */
export function createWechatTypeTextOperation(
  applicationNodeId: string,
  inputKey: WechatWorkflowInputKey,
): Extract<UiOperation, { type: 'type_text' }> {
  return {
    type: 'type_text',
    target: createWechatInputTarget(applicationNodeId),
    value: wechatInput(inputKey),
  };
}

/** 创建使用画面文字定位联系人并点击的动作。 */
export function createWechatContactClickOperation(
  applicationNodeId: string,
): Extract<UiOperation, { type: 'click' }> {
  return {
    type: 'click',
    target: {
      scope: applicationScope(applicationNodeId),
      locator: {
        type: 'query',
        query: aqlQuery(WECHAT_CONTACT_RESULT_SELECTOR, {
          contact_name: wechatInput('联系人'),
        }),
      },
      backend_policy: {
        allow: ['ocr_small', 'send_input'],
        deny: [],
        prefer: ['ocr_small', 'send_input'],
      },
    },
  };
}

/** 创建只在指定时间内等待联系人搜索结果的执行设置。 */
export function createWechatContactClickExecution(): UiExecutionPolicy {
  return {
    target_wait: {
      mode: 'bounded',
      timeout_ms: 5_000,
      poll_interval_ms: 200,
    },
  };
}

/** 创建不需要查找界面元素的键盘输入设置。 */
export function createWechatInputExecution(): UiExecutionPolicy {
  return {
    target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
  };
}

/** 创建由 UIA 或画面文字识别独立完成的一次微信界面检查。 */
export function createWechatObservation(
  applicationNodeId: string,
  source: string,
  bindings: AqlQuery['bindings'],
): ObserveSpec {
  return {
    scope: applicationScope(applicationNodeId),
    query: aqlQuery(source, bindings),
    backend_policy: {
      allow: ['ocr_small', 'windows_uia'],
      deny: [],
      prefer: ['ocr_small', 'windows_uia'],
    },
    policy: { mode: 'once' },
  };
}

/** 创建绑定微信应用节点的当前焦点输入目标。 */
function createWechatInputTarget(applicationNodeId: string): AutomationTarget {
  return {
    scope: applicationScope(applicationNodeId),
    locator: { type: 'focused' },
    backend_policy: {
      allow: ['send_input'],
      deny: [],
      prefer: ['send_input'],
    },
  };
}

/** 创建绑定到示例应用节点的作用域。 */
function applicationScope(applicationNodeId: string) {
  return {
    type: 'application' as const,
    resource: {
      producer_node_id: applicationNodeId,
      output_name: 'session',
    },
  };
}

/** 创建当前唯一持久化版本的参数化 AQL 查询。 */
function aqlQuery(
  source: string,
  bindings: AqlQuery['bindings'],
): AqlQuery {
  return { language_version: 3, source, bindings };
}
