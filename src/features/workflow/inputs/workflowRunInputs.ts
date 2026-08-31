import type {
  JsonObject,
  JsonValue,
  WorkflowInputDefinition,
} from '../model/contracts';

/** 运行输入校验成功或失败时返回的明确领域结果。 */
export type RunInputValidationResult =
  | Readonly<{ valid: true; values: JsonObject }>
  | Readonly<{ valid: false; message: string }>;

/**
 * 按当前输入声明生成完整的文本值对象。
 *
 * 声明是唯一事实来源：多余键会被移除，缺失或非文本值会回到空字符串。
 */
export function normalizeRunInputValues(
  definitions: ReadonlyArray<WorkflowInputDefinition>,
  values: JsonObject,
): JsonObject {
  return Object.fromEntries(definitions.map((definition) => [
    definition.key,
    typeof values[definition.key] === 'string' ? values[definition.key] : '',
  ])) as Record<string, JsonValue>;
}

/** 与 Runtime 的 RunInputs 边界对齐，拒绝缺失、多余或非文本值。 */
export function validateRunInputValues(
  definitions: ReadonlyArray<WorkflowInputDefinition>,
  value: unknown,
): RunInputValidationResult {
  if (!isJsonObject(value)) {
    return { valid: false, message: '运行输入必须是有效 JSON 对象。' };
  }

  const declaredKeys = new Set(definitions.map((definition) => definition.key));
  for (const definition of definitions) {
    if (!Object.prototype.hasOwnProperty.call(value, definition.key)) {
      return { valid: false, message: `缺少输入参数 '${definition.key}'。` };
    }
    if (typeof value[definition.key] !== 'string') {
      return { valid: false, message: `输入参数 '${definition.key}' 必须是文本。` };
    }
  }

  const unexpectedKey = Object.keys(value).find((key) => !declaredKeys.has(key));
  if (unexpectedKey !== undefined) {
    return { valid: false, message: `输入参数 '${unexpectedKey}' 未声明。` };
  }
  return { valid: true, values: value };
}

/** 排除 null 与数组后的 JSON 对象守卫。 */
function isJsonObject(value: unknown): value is JsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
