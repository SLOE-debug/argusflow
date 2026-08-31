import type {
  AcquirePolicy,
  ActivationPolicy,
  ApplicationSpec,
  AqlQuery,
  AutomationTarget,
  BackendKind,
  BackendPolicy,
  CleanupPolicy,
  FieldProjection,
  FieldProjectionSource,
  JsonObject,
  JsonValue,
  TargetLocator,
  TargetScope,
  UiExecutionPolicy,
  UiOperation,
  UiPostcondition,
  ValueExpr,
  ValueSource,
  WindowTitleMatcher,
} from './contracts';
import type { KeyboardKey, KeyboardModifier, KeyChord } from './inputContracts';
import { isJsonObject } from './contracts';

/** 从 canonical JSON payload 建立对象边界；缺失或错误类型立即终止模板初始化。 */
export function asObject(value: JsonValue | undefined, label = 'payload'): JsonObject {
  if (isJsonObject(value)) return value;
  throw new Error(`canonical payload field '${label}' must be an object`);
}

/** 读取必填字符串字段。 */
function asString(value: JsonValue | undefined, label: string): string {
  if (typeof value === 'string') return value;
  throw new Error(`canonical payload field '${label}' must be a string`);
}

/** 读取有限数值字段。 */
function asNumber(value: JsonValue | undefined, label: string): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  throw new Error(`canonical payload field '${label}' must be a finite number`);
}

/** 读取必填数组字段。 */
function asArray(value: JsonValue | undefined, label: string): JsonValue[] {
  if (Array.isArray(value)) return value;
  throw new Error(`canonical payload field '${label}' must be an array`);
}

/** 读取一个有固定字符串集合的协议枚举。 */
function asEnum<const Values extends readonly string[]>(
  value: JsonValue | undefined,
  values: Values,
  label: string,
): Values[number] {
  const candidate = asString(value, label);
  const supported = values.find((value) => value === candidate);
  if (supported !== undefined) return supported;
  throw new Error(`canonical payload field '${label}' has unsupported value '${candidate}'`);
}

/** 读取工作流中的结构化值表达式。 */
function asValueExpr(value: JsonValue | undefined): ValueExpr {
  const object = asObject(value, 'value_expr');
  const type = asString(object.type, 'value_expr.type');
  switch (type) {
    case 'literal':
      if (!Object.prototype.hasOwnProperty.call(object, 'value')) {
        throw new Error("canonical payload field 'value_expr.value' is required");
      }
      return { type, value: object.value };
    case 'ref':
      return {
        type,
        source: asValueSource(object.source),
        pointer: asString(object.pointer, 'value_expr.pointer'),
      };
    case 'expression':
      return { type, source: asString(object.source, 'value_expr.source') };
    default:
      throw new Error(`canonical value expression type '${type}' is unsupported`);
  }
}

/** 读取结构化值引用的来源判别联合。 */
function asValueSource(value: JsonValue | undefined): ValueSource {
  const object = asObject(value, 'value_expr.source');
  const type = asString(object.type, 'value_expr.source.type');
  switch (type) {
    case 'workflow_input':
      return { type, key: asString(object.key, 'value_expr.source.key') };
    case 'variable':
      return { type, name: asString(object.name, 'value_expr.source.name') };
    case 'node':
      return { type, node_id: asString(object.node_id, 'value_expr.source.node_id') };
    default:
      throw new Error(`canonical value source type '${type}' is unsupported`);
  }
}

/** 读取资源引用。 */
function asResourceRef(value: JsonValue | undefined): import('./contracts').ResourceRef {
  const object = asObject(value, 'target.scope.resource');
  return {
    producer_node_id: asString(object.producer_node_id, 'target.scope.resource.producer_node_id'),
    output_name: asString(object.output_name, 'target.scope.resource.output_name'),
  };
}

/** 读取目标作用域。 */
function asTargetScope(value: JsonValue | undefined): TargetScope {
  const object = asObject(value, 'target.scope');
  const type = asString(object.type, 'target.scope.type');
  if (type === 'current') return { type };
  if (type === 'application' || type === 'browser') {
    return { type, resource: asResourceRef(object.resource) };
  }
  throw new Error(`canonical target scope '${type}' is unsupported`);
}

const BACKEND_KINDS = [
  'windows_uia',
  'browser_cdp',
  'ocr_small',
  'send_input',
] as const satisfies ReadonlyArray<BackendKind>;

/** 读取后端策略的强类型数组。 */
function asBackendKinds(value: JsonValue | undefined, label: string): BackendKind[] {
  return asArray(value, label).map((item) => asEnum(item, BACKEND_KINDS, label));
}

/** 读取目标后端策略。 */
function asBackendPolicy(value: JsonValue | undefined): BackendPolicy {
  const object = asObject(value, 'target.backend_policy');
  return {
    allow: asBackendKinds(object.allow, 'target.backend_policy.allow'),
    deny: asBackendKinds(object.deny, 'target.backend_policy.deny'),
    prefer: asBackendKinds(object.prefer, 'target.backend_policy.prefer'),
  };
}

const KEYBOARD_MODIFIERS = ['control', 'alt', 'shift'] as const satisfies ReadonlyArray<KeyboardModifier>;

/** 读取按键主键。 */
function asKeyboardKey(value: JsonValue | undefined): KeyboardKey {
  const object = asObject(value, 'key_chord.key');
  const type = asString(object.type, 'key_chord.key.type');
  if (type === 'character') return { type, value: asString(object.value, 'key_chord.key.value') };
  if (type === 'enter' || type === 'escape' || type === 'tab') return { type };
  throw new Error(`canonical keyboard key '${type}' is unsupported`);
}

/** 读取组合键。 */
function asKeyChord(value: JsonValue | undefined): KeyChord {
  const object = asObject(value, 'operation.chord');
  return {
    key: asKeyboardKey(object.key),
    modifiers: asArray(object.modifiers, 'operation.chord.modifiers')
      .map((item) => asEnum(item, KEYBOARD_MODIFIERS, 'operation.chord.modifiers')),
  };
}

/** 读取目标定位器。 */
function asTargetLocator(value: JsonValue | undefined): TargetLocator {
  const object = asObject(value, 'target.locator');
  const type = asString(object.type, 'target.locator.type');
  switch (type) {
    case 'query': {
      return {
        type,
        query: asAqlQuery(object.query, 'target.locator.query'),
      };
    }
    case 'coordinate': {
      const point = asObject(object.point, 'target.locator.point');
      return {
        type,
        point: {
          x: asNumber(point.x, 'target.locator.point.x'),
          y: asNumber(point.y, 'target.locator.point.y'),
        },
      };
    }
    case 'focused':
      return { type };
    default:
      throw new Error(`canonical target locator '${type}' is unsupported`);
  }
}

/** 读取持久化 AQL 源码及其动态参数绑定。 */
function asAqlQuery(value: JsonValue | undefined, label: string): AqlQuery {
  const query = asObject(value, label);
  const languageVersion = asNumber(query.language_version, `${label}.language_version`);
  if (languageVersion !== 1 && languageVersion !== 2) {
    throw new Error("canonical AQL language_version must be 1 or 2");
  }
  const bindingsObject = query.bindings === undefined
    ? {}
    : asObject(query.bindings, `${label}.bindings`);
  return {
    language_version: languageVersion,
    source: asString(query.source, `${label}.source`),
    bindings: Object.fromEntries(
      Object.entries(bindingsObject).map(([name, expression]) => [
        name,
        asValueExpr(expression),
      ]),
    ),
  };
}

/** 读取完整自动化目标。 */
function asAutomationTarget(value: JsonValue | undefined): AutomationTarget {
  const object = asObject(value, 'operation.target');
  return {
    scope: asTargetScope(object.scope),
    locator: asTargetLocator(object.locator),
    backend_policy: asBackendPolicy(object.backend_policy),
  };
}

/** 读取 Extract 字段来源。 */
function asFieldSource(value: JsonValue | undefined): FieldProjectionSource {
  const object = asObject(value, 'operation.fields.source');
  const type = asString(object.type, 'operation.fields.source.type');
  if (type === 'text' || type === 'value' || type === 'name') return { type };
  if (type === 'property' || type === 'attribute') {
    return { type, name: asString(object.name, `operation.fields.source.${type}.name`) };
  }
  throw new Error(`canonical field source '${type}' is unsupported`);
}

/** 读取 Extract 字段投影。 */
function asFields(value: JsonValue | undefined): FieldProjection[] {
  return asArray(value, 'operation.fields').map((item) => {
    const object = asObject(item, 'operation.fields[]');
    return {
      name: asString(object.name, 'operation.fields[].name'),
      source: asFieldSource(object.source),
    };
  });
}

/** 读取 UI 操作判别联合。 */
export function asUiOperation(value: JsonValue): UiOperation {
  const object = asObject(value, 'operation');
  const type = asString(object.type, 'operation.type');
  const target = asAutomationTarget(object.target);
  switch (type) {
    case 'click':
    case 'get_text':
    case 'get_value':
    case 'collect_links':
      return { type, target };
    case 'set_value':
    case 'type_text':
      return { type, target, value: asValueExpr(object.value) };
    case 'press_key':
      return { type, target, chord: asKeyChord(object.chord) };
    case 'extract':
      return {
        type,
        target,
        cardinality: asEnum(object.cardinality, ['one', 'many'] as const, 'operation.cardinality'),
        fields: asFields(object.fields),
      };
    default:
      throw new Error(`canonical UI operation '${type}' is unsupported`);
  }
}

/** 读取目标等待策略。 */
function asWaitPolicy(value: JsonValue | undefined, label: string): UiExecutionPolicy['target_wait'] {
  const object = asObject(value, label);
  const mode = asEnum(object.mode, ['none', 'bounded'] as const, `${label}.mode`);
  return {
    mode,
    timeout_ms: asNumber(object.timeout_ms, `${label}.timeout_ms`),
    poll_interval_ms: asNumber(object.poll_interval_ms, `${label}.poll_interval_ms`),
  };
}

/** 读取 UI 后置条件。 */
function asPostcondition(value: JsonValue | undefined): UiPostcondition | null {
  if (value === null) return null;
  const object = asObject(value, 'execution.postcondition');
  const type = asString(object.type, 'execution.postcondition.type');
  if (type === 'match_added') {
    return {
      type,
      query: asAqlQuery(object.query, 'execution.postcondition.query'),
      stable_context: asArray(
        object.stable_context,
        'execution.postcondition.stable_context',
      ).map((query, index) => asAqlQuery(
        query,
        `execution.postcondition.stable_context[${index}]`,
      )),
    };
  }
  if (type === 'match_present') {
    return {
      type,
      query: asAqlQuery(object.query, 'execution.postcondition.query'),
    };
  }
  throw new Error(`canonical postcondition '${type}' is unsupported`);
}

/** 读取完整 UI 执行策略。 */
export function asUiExecutionPolicy(value: JsonValue | undefined): UiExecutionPolicy {
  const object = asObject(value, 'execution');
  return {
    target_wait: asWaitPolicy(object.target_wait, 'execution.target_wait'),
    postcondition_wait: asWaitPolicy(object.postcondition_wait, 'execution.postcondition_wait'),
    postcondition: asPostcondition(object.postcondition),
  };
}

/** 将 canonical application payload 收窄到应用资源模型。 */
export function asApplicationSpec(value: JsonValue | undefined): ApplicationSpec {
  const object = asObject(value, 'application.spec');
  const windowTitle = asObject(object.window_title, 'application.spec.window_title');
  const windowTitleType = asEnum(windowTitle.type, ['equal', 'contains'] as const, 'application.spec.window_title.type');
  const matcher: WindowTitleMatcher = {
    type: windowTitleType,
    value: asString(windowTitle.value, 'application.spec.window_title.value'),
  };
  return {
    executable_path: asString(object.executable_path, 'application.spec.executable_path'),
    arguments: asArray(object.arguments, 'application.spec.arguments')
      .map((item) => asString(item, 'application.spec.arguments[]')),
    window_title: matcher,
    acquire_policy: asEnum(
      object.acquire_policy,
      ['attach_or_start', 'attach_only', 'always_start_new'] as const satisfies ReadonlyArray<AcquirePolicy>,
      'application.spec.acquire_policy',
    ),
    launch_timeout_ms: asNumber(object.launch_timeout_ms, 'application.spec.launch_timeout_ms'),
    cleanup_policy: asEnum(
      object.cleanup_policy,
      ['leave_running', 'close_if_started_by_workflow', 'always_close'] as const satisfies ReadonlyArray<CleanupPolicy>,
      'application.spec.cleanup_policy',
    ),
    activation_policy: asEnum(
      object.activation_policy,
      ['none', 'best_effort', 'required'] as const satisfies ReadonlyArray<ActivationPolicy>,
      'application.spec.activation_policy',
    ),
  };
}

/** 只改写 operation.target.scope.resource 的资源生产节点，不扫描业务字符串。 */
export function rewriteCanonicalOperationReferences(
  value: JsonValue,
  applicationNodeId: string,
): UiOperation {
  const operation = asUiOperation(value);
  const scope = operation.target.scope;
  if (scope.type !== 'application' || scope.resource.producer_node_id !== 'wechat_application') {
    return operation;
  }
  return {
    ...operation,
    target: {
      ...operation.target,
      scope: {
        ...scope,
        resource: {
          ...scope.resource,
          producer_node_id: applicationNodeId,
        },
      },
    },
  };
}
