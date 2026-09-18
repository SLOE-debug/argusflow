import {
  isSpatialPreview,
  type JsonValue,
  type Value,
} from "../../../../features/workflow";

/** 列表摘要只描述实际内容，不暴露传输协议中的类型标记。 */
export function resultSummary(value: Value): string {
  if (isSpatialPreview(value)) return "位置示意";
  switch (value.type) {
    case "text":
      return value.value || "空白内容";
    case "bool":
      return value.value ? "是" : "否";
    case "int":
    case "float":
      return String(value.value);
    case "list":
      return `${value.value.length} 项内容`;
    case "record":
      return `${Object.keys(value.value).length} 项信息`;
    case "optional":
      return value.value === null ? "无内容" : resultSummary(value.value);
  }
}

/** 复制时剥离类型外壳；大整数保留原始字符串，避免精度损失。 */
function plainValue(value: Value): JsonValue {
  switch (value.type) {
    case "record":
      return Object.fromEntries(
        Object.entries(value.value).map(([name, item]) => [
          name,
          plainValue(item),
        ]),
      );
    case "list":
      return value.value.map(plainValue);
    case "optional":
      return value.value === null ? null : plainValue(value.value);
    default:
      return value.value;
  }
}

/** 单项文本直接复制，多项内容以不含协议包装的 JSON 保留层次。 */
export function resultClipboard(value: Value): string {
  const content = plainValue(value);
  return typeof content === "object"
    ? JSON.stringify(content, null, 2)
    : String(content);
}
