import type { BackendPolicyPreset } from '../nodes/workflowAction';
import {
  changeTargetLocator,
  changeTargetLocatorKind,
  changeTargetScope,
  resolveBackendPolicyPreset,
} from '../nodes/workflowAction';
import type {
  TargetScope,
  UiExecutionPolicy,
  UiOperation,
  ValueExpr,
} from '../model/contracts';
import type {
  WorkflowResourceCatalog,
  WorkflowResourceOption,
} from '../values/workflowResourceCatalog';
import {
  INTENT_CONTROL_ROLE_LABELS,
  type ActionInspectorViewModel,
  type ActionLocationViewModel,
  type ActionTargetIntent,
  type IntentControlRole,
  type IntentMatchMode,
} from './actionInspectorTypes';
import {
  changeQueryControlRole,
  changeQueryTargetMatch,
  changeQueryTargetText,
  changeQueryTargetType,
  formatIntentValueExpression,
  readQueryTargetIntent,
} from './actionQueryIntent';

/** 用户语言动作名称的唯一词典。 */
export const ACTION_INTENT_LABELS: Readonly<Record<UiOperation['type'], string>> = {
  click: '单击',
  set_value: '输入文本',
  press_key: '按键',
  type_text: '键入文本',
};

/** 把现有 UI operation/execution 映射为不泄漏引擎字段的 Intent ViewModel。 */
export function buildActionInspectorViewModel(
  operation: UiOperation,
  execution: UiExecutionPolicy,
  resourceCatalog: WorkflowResourceCatalog,
  invalid = false,
): ActionInspectorViewModel {
  const location = resolveActionLocation(operation.target.scope, resourceCatalog);
  const target = resolveActionTarget(operation);
  const backendPreset = resolveBackendPolicyPreset(operation.target.backend_policy);
  const waitPolicy = execution.target_wait;
  return {
    summary: formatActionSummary(operation, location, target),
    actionLabel: ACTION_INTENT_LABELS[operation.type],
    location,
    target,
    targetStatus: invalid
      ? { type: 'invalid', message: '目标配置需要修改' }
      : resolveInitialTargetStatus(target),
    waitForTarget: waitPolicy.mode === 'bounded',
    timeoutSeconds: waitPolicy.timeout_ms / 1_000,
    retryIntervalMs: waitPolicy.poll_interval_ms,
    locatorEngineLabel: formatLocatorEngine(backendPreset, target),
    actionEngineLabel: formatActionEngine(operation, backendPreset),
  };
}

/** 使用统一应用/窗口选择值更新 engine scope。 */
export function changeActionLocation(
  operation: UiOperation,
  value: ActionLocationViewModel['value'],
): UiOperation {
  if (value === 'current') return changeTargetScope(operation, { type: 'current' });
  const separatorIndex = value.indexOf(':');
  const type = value.slice(0, separatorIndex);
  const producerNodeId = value.slice(separatorIndex + 1);
  if (type !== 'application' && type !== 'browser') return operation;
  return changeTargetScope(operation, {
    type,
    resource: { producer_node_id: producerNodeId, output_name: 'session' },
  });
}

/** 切换用户语义目标类型，并反向写回现有 locator/AQL。 */
export function changeActionTargetType(
  operation: UiOperation,
  type: 'text' | 'control' | 'web' | 'coordinate',
): UiOperation {
  if (type === 'coordinate') return changeTargetLocatorKind(operation, 'coordinate');
  const queryOperation = operation.target.locator.type === 'query'
    ? operation
    : changeTargetLocatorKind(operation, 'query');
  if (queryOperation.target.locator.type !== 'query') return operation;
  return changeTargetLocator(queryOperation, {
    ...queryOperation.target.locator,
    query: changeQueryTargetType(queryOperation.target.locator.query, type),
  });
}

/** 更新文字、控件名称或网页 CSS 选择器。 */
export function changeActionTargetText(operation: UiOperation, text: string): UiOperation {
  if (operation.target.locator.type !== 'query') return operation;
  return changeTargetLocator(operation, {
    ...operation.target.locator,
    query: changeQueryTargetText(operation.target.locator.query, text),
  });
}

/** 更新基础目标的匹配方式。 */
export function changeActionTargetMatch(
  operation: UiOperation,
  match: IntentMatchMode,
): UiOperation {
  if (operation.target.locator.type !== 'query') return operation;
  return changeTargetLocator(operation, {
    ...operation.target.locator,
    query: changeQueryTargetMatch(operation.target.locator.query, match),
  });
}

/** 更新基础控件角色。 */
export function changeActionControlRole(
  operation: UiOperation,
  role: IntentControlRole,
): UiOperation {
  if (operation.target.locator.type !== 'query') return operation;
  return changeTargetLocator(operation, {
    ...operation.target.locator,
    query: changeQueryControlRole(operation.target.locator.query, role),
  });
}

/** 把当前 scope 适配为应用优先的选择器值和展示内容。 */
function resolveActionLocation(
  scope: TargetScope,
  catalog: WorkflowResourceCatalog,
): ActionLocationViewModel {
  if (scope.type === 'current') {
    return {
      value: 'current',
      label: '当前窗口',
      sourceLabel: null,
      unavailableReason: null,
    };
  }
  const option = catalog[scope.type].find(({ nodeId }) => (
    nodeId === scope.resource.producer_node_id
  ));
  return resourceLocation(scope.type, scope.resource.producer_node_id, option);
}

/** 组装资源位置；失效引用仍保留原始 ID，不能伪装成未选择。 */
function resourceLocation(
  type: 'application' | 'browser',
  nodeId: string,
  option: WorkflowResourceOption | undefined,
): ActionLocationViewModel {
  const resourceLabel = option?.resourceLabel ?? nodeId;
  return {
    value: `${type}:${nodeId}`,
    label: resourceLabel || '未选择',
    sourceLabel: option?.nodeLabel ?? null,
    unavailableReason: option
      ? option.available ? null : option.unavailableReason ?? '当前不可用'
      : '来源节点不存在',
  };
}

/** 按 locator 判别联合构造目标意图。 */
function resolveActionTarget(operation: UiOperation): ActionTargetIntent {
  switch (operation.target.locator.type) {
    case 'query':
      return readQueryTargetIntent(operation.target.locator.query);
    case 'coordinate':
      return {
        type: 'coordinate',
        x: operation.target.locator.point.x,
        y: operation.target.locator.point.y,
      };
    case 'focused':
      return { type: 'focus' };
  }
}

/** 没有编辑态检查 API 时只陈述已知事实，不伪造匹配数量。 */
function resolveInitialTargetStatus(
  target: ActionTargetIntent,
): ActionInspectorViewModel['targetStatus'] {
  switch (target.type) {
    case 'coordinate':
      return { type: 'configured', message: '已设置屏幕坐标' };
    case 'focus':
      return { type: 'configured', message: '将使用当前焦点' };
    case 'advanced':
    case 'control':
    case 'text':
    case 'web':
      return { type: 'unchecked' };
  }
}

/** 生成可在画布和属性面板复用的人类可读任务摘要。 */
function formatActionSummary(
  operation: UiOperation,
  location: ActionLocationViewModel,
  target: ActionTargetIntent,
): string {
  const place = `在「${location.label}」中`;
  switch (operation.type) {
    case 'click':
      return `${place}找到${formatTargetSummary(target)}并单击。`;
    case 'set_value':
      return `${place}找到${formatTargetSummary(target)}并输入${formatQuotedValue(operation.value)}。`;
    case 'press_key':
      return `${place}向当前焦点发送按键。`;
    case 'type_text':
      return `${place}向当前焦点键入${formatQuotedValue(operation.value)}。`;
  }
}

/** 为不同语义目标生成一句话中的宾语。 */
function formatTargetSummary(target: ActionTargetIntent): string {
  switch (target.type) {
    case 'text':
      return target.value.source === 'binding'
        ? `由${target.value.text}指定的文字`
        : `文字「${target.value.text || '未设置'}」`;
    case 'control':
      return target.value.source === 'binding'
        ? `由${target.value.text}指定的${INTENT_CONTROL_ROLE_LABELS[target.role]}`
        : `${INTENT_CONTROL_ROLE_LABELS[target.role]}「${target.value.text || '未命名'}」`;
    case 'web':
      return `网页元素「${target.selector || '未设置'}」`;
    case 'coordinate':
      return `坐标（${target.x}, ${target.y}）`;
    case 'focus':
      return '当前焦点';
    case 'advanced':
      return '符合查找规则的目标';
  }
}

/** 值表达式在任务摘要中始终带中文引号。 */
function formatQuotedValue(value: ValueExpr): string {
  return `「${formatIntentValueExpression(value)}」`;
}

/** “执行方式”区域中的定位引擎名称。 */
function formatLocatorEngine(
  preset: BackendPolicyPreset,
  target: ActionTargetIntent,
): string {
  if (target.type === 'coordinate' || target.type === 'focus') return '无需定位';
  switch (preset) {
    case 'auto':
      return '自动（推荐）';
    case 'windows_uia':
      return 'Windows 控件（UIA）';
    case 'browser_cdp':
      return '网页元素（CDP）';
    case 'ocr_small':
      return '屏幕文字（OCR）';
    case 'send_input':
      return '无需定位';
  }
}

/** “执行方式”区域中的动作执行方式只陈述当前策略能够保证的内容。 */
function formatActionEngine(
  operation: UiOperation,
  preset: BackendPolicyPreset,
): string {
  if (operation.type === 'press_key' || operation.type === 'type_text') {
    return '模拟键盘输入';
  }
  if (preset === 'browser_cdp') return '网页动作';
  if (preset === 'windows_uia') return '控件动作';
  if (preset === 'ocr_small') return '模拟鼠标操作';
  return '自动（推荐）';
}
