import type {
  AutomationTarget,
  BackendKind,
  BackendPolicy,
  TargetLocator,
  TargetLocatorKind,
  TargetScope,
  UiOperation,
  UiOperationKind,
  UiExecutionPolicy,
  ValueExpr,
} from '../model/contracts';
import type { KeyChord } from '../model/inputContracts';

/** 新建 UI 节点使用的可执行 AQL 示例。 */
export const DEFAULT_ACTION_AQL_SOURCE = 'first(button(name = "确定"))';

/** 创建语义查询目标默认使用的快速有界等待策略。 */
export function createDefaultUiExecutionPolicy(): UiExecutionPolicy {
  return {
    target_wait: {
      mode: 'bounded',
      timeout_ms: 5_000,
      poll_interval_ms: 100,
    },
    postcondition_wait: {
      mode: 'bounded',
      timeout_ms: 5_000,
      poll_interval_ms: 150,
    },
    postcondition: null,
  };
}

/** 根据定位成本建立等待策略；坐标目标没有元素出现语义。 */
export function createTargetWaitPolicy(
  locatorKind: TargetLocatorKind,
): UiExecutionPolicy['target_wait'] {
  switch (locatorKind) {
    case 'query':
      return createDefaultUiExecutionPolicy().target_wait;
    case 'visual':
      return { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 300 };
    case 'coordinate':
    case 'focused':
      return { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 };
  }
}

/** 创建默认点击操作；作用域、定位语义与后端偏好相互独立。 */
export function createDefaultUiOperation(): UiOperation {
  return {
    type: 'click',
    target: {
      scope: { type: 'current' },
      locator: createTargetLocator('query'),
      backend_policy: createBackendPolicy('auto'),
    },
  };
}

/** 保留目标配置并切换语义操作类型。 */
export function changeUiOperationKind(
  operation: UiOperation,
  kind: UiOperationKind,
): UiOperation {
  /** 元素动作不能沿用只对键盘输入有意义的当前焦点定位。 */
  const elementTarget = operation.target.locator.type === 'focused'
    ? {
        ...operation.target,
        locator: createTargetLocator('query'),
        backend_policy: createBackendPolicy('auto'),
      }
    : operation.target;
  /** 键盘动作必须显式使用当前焦点和 SendInput。 */
  const inputTarget = {
    ...operation.target,
    scope: operation.target.scope.type === 'browser'
      ? { type: 'current' as const }
      : operation.target.scope,
    locator: createTargetLocator('focused'),
    backend_policy: createBackendPolicy('send_input'),
  };
  switch (kind) {
    case 'click':
      return { type: kind, target: elementTarget };
    case 'set_value':
      return {
        type: kind,
        target: elementTarget,
        value: operation.type === 'set_value' || operation.type === 'type_text'
          ? operation.value
          : { type: 'literal', value: '' },
      };
    case 'press_key':
      return {
        type: kind,
        target: inputTarget,
        chord: operation.type === 'press_key'
          ? operation.chord
          : { key: { type: 'enter' }, modifiers: [] },
      };
    case 'type_text':
      return {
        type: kind,
        target: inputTarget,
        value: operation.type === 'set_value' || operation.type === 'type_text'
          ? operation.value
          : { type: 'literal', value: '' },
      };
    case 'get_text':
      return { type: kind, target: elementTarget };
    case 'get_value':
      return { type: kind, target: elementTarget };
    case 'extract':
      return {
        type: kind,
        target: elementTarget.locator.type === 'query'
          ? elementTarget
          : {
              ...elementTarget,
              locator: createTargetLocator('query'),
            },
        cardinality: 'many',
        fields: [{ name: 'text', source: { type: 'text' } }],
      };
    case 'collect_links':
      return {
        type: kind,
        target: {
          ...elementTarget,
          locator: elementTarget.locator.type === 'query'
            ? elementTarget.locator
            : createTargetLocator('query'),
          backend_policy: createBackendPolicy('browser_cdp'),
        },
      };
  }
}

/** 以新目标替换 UI 操作目标，同时保留专属参数。 */
export function replaceAutomationTarget(
  operation: UiOperation,
  target: AutomationTarget,
): UiOperation {
  switch (operation.type) {
    case 'click':
      return { type: operation.type, target };
    case 'set_value':
      return { ...operation, target };
    case 'press_key':
      return { ...operation, target };
    case 'type_text':
      return { ...operation, target };
    case 'get_text':
      return { type: operation.type, target };
    case 'get_value':
      return { type: operation.type, target };
    case 'extract':
      return { ...operation, target };
    case 'collect_links':
      return { type: operation.type, target };
  }
}

/** 切换定位方式；非语义目标固定由 Planner 自动选择。 */
export function changeTargetLocatorKind(
  operation: UiOperation,
  kind: TargetLocatorKind,
): UiOperation {
  return replaceAutomationTarget(operation, {
    ...operation.target,
    locator: createTargetLocator(kind),
    backend_policy: kind === 'query'
      ? operation.target.backend_policy
      : createBackendPolicy(kind === 'focused' ? 'send_input' : 'auto'),
  });
}

/** 更新 UI 操作的逻辑资源作用域。 */
export function changeTargetScope(
  operation: UiOperation,
  scope: TargetScope,
): UiOperation {
  return replaceAutomationTarget(operation, { ...operation.target, scope });
}

/** 编辑器可直接表达的后端策略预设。 */
export type BackendPolicyPreset = 'auto' | Extract<
  BackendKind,
  'windows_uia' | 'browser_cdp' | 'send_input'
>;

/** 更新动作目标的后端策略预设。 */
export function changeBackendPolicy(
  operation: UiOperation,
  preset: BackendPolicyPreset,
): UiOperation {
  return replaceAutomationTarget(operation, {
    ...operation.target,
    backend_policy: createBackendPolicy(preset),
  });
}

/** 把编辑器预设转换为运行时开放集合策略。 */
export function createBackendPolicy(preset: BackendPolicyPreset): BackendPolicy {
  if (preset === 'auto') {
    return { allow: [], deny: [], prefer: [] };
  }
  return { allow: [preset], deny: [], prefer: [preset] };
}

/** 将受支持的策略还原为编辑器预设；其它注册策略保留为自动展示。 */
export function resolveBackendPolicyPreset(policy: BackendPolicy): BackendPolicyPreset {
  if (policy.allow.length === 1
    && policy.deny.length === 0
    && policy.prefer[0] === policy.allow[0]
    && (
      policy.allow[0] === 'windows_uia'
      || policy.allow[0] === 'browser_cdp'
      || policy.allow[0] === 'send_input'
    )) {
    return policy.allow[0];
  }
  return 'auto';
}

/** 更新操作的定位契约。 */
export function changeTargetLocator(
  operation: UiOperation,
  locator: TargetLocator,
): UiOperation {
  return replaceAutomationTarget(operation, {
    ...operation.target,
    locator,
  });
}

/** 更新 SetValue 的值表达式。 */
export function changeSetValue(
  operation: Extract<UiOperation, { type: 'set_value' }>,
  value: ValueExpr,
): UiOperation {
  return { ...operation, value };
}

/** 更新物理文本输入的值表达式。 */
export function changeTypeText(
  operation: Extract<UiOperation, { type: 'type_text' }>,
  value: ValueExpr,
): UiOperation {
  return { ...operation, value };
}

/** 更新组合键，同时保留当前焦点目标。 */
export function changeKeyChord(
  operation: Extract<UiOperation, { type: 'press_key' }>,
  chord: KeyChord,
): UiOperation {
  return { ...operation, chord };
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
      return {
        type: 'visual',
        query: {
          text: { type: 'literal', value: '确定' },
          exact: true,
          region: null,
        },
      };
    case 'coordinate':
      return { type: 'coordinate', point: { x: 0, y: 0 } };
    case 'focused':
      return { type: 'focused' };
  }
}
