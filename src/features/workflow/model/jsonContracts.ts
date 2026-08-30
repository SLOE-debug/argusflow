/** 可在前后端无损传递且不要求调用方开放集合写权限的 JSON 值。 */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | ReadonlyArray<JsonValue>
  | { readonly [key: string]: JsonValue };

/** 可按字符串键读取、但不能由调用方就地改写的 JSON 对象。 */
export type JsonObject = { readonly [key: string]: JsonValue };

/** 在已知 JSON 值中排除 null 与数组，并建立可索引对象边界。 */
export function isJsonObject(value: JsonValue | undefined): value is JsonObject {
  return value !== null && value !== undefined && typeof value === 'object'
    && !Array.isArray(value);
}
