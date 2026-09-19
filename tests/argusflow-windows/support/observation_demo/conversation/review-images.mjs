/** 显式追加视觉交叉核对，不把参考答案或执行计划传给模型。 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { loadEvidence } from "./evidence.mjs";
import { EvidenceImages } from "./images.mjs";
import { restoreOverviewConversation } from "./history.mjs";
import { evidenceTools, dispatchTool } from "./tools.mjs";
import { makeClient } from "./client.mjs";
const directory = resolve(process.argv[2]);
const prior = resolve(directory, "plus-multi-01");
const output = resolve(directory, "plus-visual-review");
await mkdir(output);
const recording = await loadEvidence(directory);
const request = await restoreOverviewConversation(prior, 4);
const answer = JSON.parse(
  await readFile(resolve(prior, "response-4.json"), "utf8"),
).choices[0].message;
request.messages.push({ role: "assistant", content: answer.content });
request.messages.push({
  role: "user",
  content:
    "追加视觉交叉核对：请使用 view_change 查看你最后两处原生文本选区结论的前后局部图，再判断是否与 UIA 结论矛盾。你自己选择要查询的 sample ID。只输出 JSON 核对结果、证据引用和不确定性，不重写整个 workflow。",
});
const client = await makeClient(output, request.model);
const images = new EvidenceImages(recording, resolve(output, "images"));
const first = await client(
  request.messages,
  evidenceTools.filter((t) => t.function.name === "view_change"),
  1,
);
const message = first.choices[0].message;
if (!message.tool_calls?.length || message.tool_calls.length > 3)
  throw Error("模型未提出限定数量的图像查询");
request.messages.push({
  role: "assistant",
  content: message.content ?? null,
  tool_calls: message.tool_calls,
});
const supplements = [];
for (const [index, call] of message.tool_calls.entries()) {
  const { images: frames, ...result } = await dispatchTool(
    recording,
    images,
    call,
  );
  const metadata = frames.map(({ data, file, ...rest }) => rest);
  await writeFile(
    resolve(output, `tool-${index + 1}.json`),
    JSON.stringify({ call, result, images: metadata }, null, 2),
  );
  request.messages.push({
    role: "tool",
    tool_call_id: call.id,
    content: JSON.stringify({ ...result, images: metadata }),
  });
  for (const frame of frames)
    supplements.push(
      {
        type: "text",
        text: `工具 ${call.id} 的 ${frame.sample_id}；crop=${JSON.stringify(frame.crop)}，像素尺寸${frame.width}×${frame.height}`,
      },
      {
        type: "image_url",
        image_url: { url: `data:image/png;base64,${frame.data}` },
      },
    );
}
if (supplements.length)
  request.messages.push({ role: "user", content: supplements });
const final = await client(request.messages, [], 2);
const result = JSON.parse(final.choices[0].message.content);
await writeFile(
  resolve(output, "review.json"),
  JSON.stringify(result, null, 2),
);
await writeFile(
  resolve(output, "metrics.json"),
  JSON.stringify(
    {
      model: request.model,
      purpose: "独立计费的追加视觉核对；不计入主对照实验",
      additional_images: images.count,
      additional_image_pixels: images.pixels,
      calls: message.tool_calls,
      usage: [first.usage, final.usage],
    },
    null,
    2,
  ),
);
