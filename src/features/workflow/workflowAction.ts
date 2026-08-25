import type {
  AutomationAction,
  AutomationActionKind,
  AutomationTarget,
  BackendPreference,
  TargetLocator,
  TargetLocatorKind,
} from './contracts';

/** 新建 Action 节点使用的可执行 AQL 示例。 */
export const DEFAULT_ACTION_AQL_SOURCE = 'first(button(name = "确定"))';

/** 创建默认点击动作；目标语义与后端偏好保持相互独立。 */
export function createDefaultAutomationAction(): AutomationAction {
  return {
    type: 'click',
    target: {
      locator: createTargetLocator('query'),
      backend_preference: 'auto',
    },
  };
}

/** 保留目标配置并切换动作类型。 */
export function changeAutomationActionKind(
  action: AutomationAction,
  kind: AutomationActionKind,
): AutomationAction {
  if (kind === 'click') {
    return { type: 'click', target: action.target };
  }

  return {
    type: 'set_value',
    target: action.target,
    value: action.type === 'set_value' ? action.value : '',
  };
}

/** 以新目标替换动作目标，同时保留动作专属参数。 */
export function replaceAutomationTarget(
  action: AutomationAction,
  target: AutomationTarget,
): AutomationAction {
  return action.type === 'click'
    ? { type: 'click', target }
    : { type: 'set_value', target, value: action.value };
}

/** 切换定位方式；非 AQL 目标只能由 planner 自动选择后端。 */
export function changeTargetLocatorKind(
  action: AutomationAction,
  kind: TargetLocatorKind,
): AutomationAction {
  const target: AutomationTarget = {
    locator: createTargetLocator(kind),
    backend_preference: kind === 'query'
      ? action.target.backend_preference
      : 'auto',
  };
  return replaceAutomationTarget(action, target);
}

/** 更新动作目标的后端偏好。 */
export function changeBackendPreference(
  action: AutomationAction,
  backendPreference: BackendPreference,
): AutomationAction {
  return replaceAutomationTarget(action, {
    ...action.target,
    backend_preference: backendPreference,
  });
}

/** 更新动作的定位契约。 */
export function changeTargetLocator(
  action: AutomationAction,
  locator: TargetLocator,
): AutomationAction {
  return replaceAutomationTarget(action, {
    ...action.target,
    locator,
  });
}

/** 更新 SetValue 文本；调用方应只在该动作分支中使用。 */
export function changeSetValueText(
  action: Extract<AutomationAction, { type: 'set_value' }>,
  value: string,
): AutomationAction {
  return { ...action, value };
}

/** 为指定定位类别建立字段完整的默认契约。 */
function createTargetLocator(kind: TargetLocatorKind): TargetLocator {
  switch (kind) {
    case 'query':
      return {
        type: 'query',
        query: { language_version: 1, source: DEFAULT_ACTION_AQL_SOURCE },
      };
    case 'visual':
      return { type: 'visual', query: { text: '确定', exact: true } };
    case 'coordinate':
      return { type: 'coordinate', point: { x: 0, y: 0 } };
  }
}
