/** 同一 messages 历史中的 assistant/tool 往返；有限轮数，不自动执行推导。 */
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { makeClient } from "./client.mjs";
import { EvidenceImages } from "./images.mjs";
import { evidenceTools, dispatchTool } from "./tools.mjs";
import { systemPrompt, initialPrompt } from "./prompt.mjs";
import { saveResult } from "../prompts/result.mjs";

export async function runDialogue(recording, output, mode, model) {
  await mkdir(output, { recursive: false });
  const images = new EvidenceImages(recording, resolve(output, "images"));
  const overview = await images.overview();
  const imageContent = (image) => ({
    type: "image_url",
    image_url: { url: `data:image/png;base64,${image.data}` },
  });
  const messages = [
    { role: "system", content: systemPrompt },
    {
      role: "user",
      content: [
        { type: "text", text: initialPrompt(recording) },
        {
          type: "text",
          text: `开场窗口缩略图 ${overview[0].sample_id}，${overview[0].width}×${overview[0].height}；不代表后续状态。`,
        },
        imageContent(overview[0]),
      ],
    },
  ];
  const client = await makeClient(output, model);
  const log = [];
  let toolCount = 0,
    imagePixelsSent = 0,
    cumulativeTokens = 0;
  const maxRounds = mode === "single" ? 1 : 4;
  for (let round = 1; round <= maxRounds; round++) {
    imagePixelsSent += images.pixels;
    if (imagePixelsSent > 5_000_000 || cumulativeTokens > 60_000)
      throw Error("累计推理预算耗尽");
    const allowTools = mode === "multi" && round < maxRounds && toolCount < 6;
    if (mode === "multi" && !allowTools)
      messages.push({
        role: "user",
        content:
          "查询预算结束。请基于已有证据输出最终 JSON，无法确定的部分放 unresolved，不得补造。",
      });
    const response = await client(
      messages,
      allowTools ? evidenceTools : [],
      round,
    );
    cumulativeTokens += response.usage?.total_tokens ?? 0;
    const message = response.choices[0].message;
    log.push({
      round,
      usage: response.usage,
      tool_calls: message.tool_calls ?? [],
      image_pixels_submitted: images.pixels,
    });
    messages.push({
      role: "assistant",
      content: message.content ?? null,
      ...(message.tool_calls?.length ? { tool_calls: message.tool_calls } : {}),
    });
    if (!message.tool_calls?.length) {
      const result = await saveResult(output, JSON.parse(message.content));
      const metrics = {
        model,
        mode,
        rounds: round,
        tool_calls: toolCount,
        images: images.count,
        unique_image_pixels: images.pixels,
        cumulative_image_pixels: imagePixelsSent,
        total_tokens: cumulativeTokens,
        log,
      };
      await writeFile(
        resolve(output, "metrics.json"),
        JSON.stringify(metrics, null, 2),
      );
      return { ...result, metrics };
    }
    if (!allowTools) throw Error("工具被禁用时模型仍请求工具");
    const supplements = [];
    for (const call of message.tool_calls) {
      let result;
      try {
        if (++toolCount > 6) throw Error("六次工具调用预算耗尽");
        result = await dispatchTool(recording, images, call);
      } catch (error) {
        result = { error: String(error), images: [] };
      }
      const { images: frames, ...facts } = result;
      const metadata = frames.map(({ data, file, ...rest }) => rest);
      const toolResult = { ...facts, images: metadata };
      await writeFile(
        resolve(output, `tool-${toolCount}.json`),
        JSON.stringify({ call, result: toolResult }, null, 2),
      );
      messages.push({
        role: "tool",
        tool_call_id: call.id,
        content: JSON.stringify(toolResult),
      });
      for (const frame of frames)
        supplements.push(
          {
            type: "text",
            text: `工具 ${call.id} 返回的变化区域图 ${frame.sample_id}，原窗口裁剪范围 ${JSON.stringify(frame.crop)}，${frame.width}×${frame.height}。`,
          },
          imageContent(frame),
        );
    }
    // 工具文本遵循 tool_call_id；图片作为紧邻的 user 多模态消息关联到同一调用。
    if (supplements.length)
      messages.push({ role: "user", content: supplements });
  }
  throw Error("达到轮数上限，未生成完整 workflow");
}
