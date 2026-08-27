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
} from './contracts';

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
  switch (kind) {
    case 'click':
      return { type: kind, target: operation.target };
    case 'set_value':
      return {
        type: kind,
        target: operation.target,
        value: operation.type === 'set_value'
          ? operation.value
          : { type: 'literal', value: '' },
      };
    case 'get_text':
      return { type: kind, target: operation.target };
    case 'get_value':
      return { type: kind, target: operation.target };
    case 'extract':
      return {
        type: kind,
        target: operation.target.locator.type === 'query'
          ? operation.target
          : {
              ...operation.target,
              locator: createTargetLocator('query'),
            },
        cardinality: 'many',
        fields: [{ name: 'text', source: { type: 'text' } }],
      };
    case 'collect_links':
      return {
        type: kind,
        target: {
          ...operation.target,
          locator: operation.target.locator.type === 'query'
            ? operation.target.locator
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
      : createBackendPolicy('auto'),
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
  'windows_uia' | 'browser_cdp'
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
    && (policy.allow[0] === 'windows_uia' || policy.allow[0] === 'browser_cdp')) {
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
