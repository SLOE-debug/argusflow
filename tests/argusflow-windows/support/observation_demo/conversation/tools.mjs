/** 模型可调用的白名单查询，只能补读现有证据，不能操作桌面或网络。 */
import { inspectFacts } from "./evidence.mjs";

export const evidenceTools = [
  {
    type: "function",
    function: {
      name: "inspect_evidence",
      description:
        "按时间线 ID 查询原始 UIA 文档/选区、剪贴板或 CDP 事实。请求最多24个ID，每页返回6条和remaining_ids；有剩余时可继续查询。",
      parameters: {
        type: "object",
        properties: {
          ids: {
            type: "array",
            items: { type: "string" },
            minItems: 1,
            maxItems: 24,
          },
        },
        required: ["ids"],
        additionalProperties: false,
      },
    },
  },
  {
    type: "function",
    function: {
      name: "view_change",
      description:
        "查看 sample ID 与同窗口前一帧之间的变化区域前后小图。context=change 仅差分；看不全时可用 text_line 包含相交文字行。禁止整屏，最多三对，单图最大640px宽及180k像素。",
      parameters: {
        type: "object",
        properties: {
          sample_id: { type: "string" },
          context: { type: "string", enum: ["change", "text_line"] },
        },
        required: ["sample_id"],
        additionalProperties: false,
      },
    },
  },
];

export async function dispatchTool(recording, images, call) {
  if (
    typeof call.function?.arguments !== "string" ||
    call.function.arguments.length > 2048
  )
    throw Error("工具参数无效或超限");
  const args = JSON.parse(call.function.arguments);
  switch (call.function.name) {
    case "inspect_evidence":
      if (Object.keys(args).some((k) => k !== "ids"))
        throw Error("未知工具参数");
      if (!Array.isArray(args.ids) || args.ids.length > 24)
        throw Error("查询参数超出上限");
      return {
        facts: inspectFacts(recording, args.ids.slice(0, 6)),
        remaining_ids: args.ids.slice(6),
        images: [],
      };
    case "view_change":
      if (Object.keys(args).some((k) => !["sample_id", "context"].includes(k)))
        throw Error("未知工具参数");
      return images.change(args.sample_id, args.context);
    default:
      throw Error("工具不在只读白名单");
  }
}
