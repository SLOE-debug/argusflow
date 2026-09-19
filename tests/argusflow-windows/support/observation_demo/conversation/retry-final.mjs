/** 输出截断后的显式最终轮重试；复用完整对话，累计计入失败轮用量。 */
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { makeClient } from "./client.mjs";
import { restoreOverviewConversation } from "./history.mjs";
import { saveResult } from "../prompts/result.mjs";
const directory = resolve(process.argv[2]);
const read = async (file) =>
  JSON.parse(await readFile(resolve(directory, file), "utf8"));
const failed = await read("response-4.json");
if (failed.choices?.[0]?.finish_reason !== "length")
  throw Error("仅允许对输出长度截断重试");
const request = await restoreOverviewConversation(directory, 4);
const client = await makeClient(directory, request.model);
const result = await client(request.messages, [], 5, 8000);
await saveResult(directory, JSON.parse(result.choices[0].message.content));
const overview = (await read("images/overview.json"))[0];
const log = [];
for (let round = 1; round <= 5; round++) {
  const response = await read(`response-${round}.json`);
  log.push({
    round,
    usage: response.usage,
    tool_calls: response.choices[0].message.tool_calls ?? [],
    image_pixels_submitted: overview.width * overview.height,
    finish_reason: response.choices[0].finish_reason,
  });
}
await writeFile(
  resolve(directory, "metrics.json"),
  JSON.stringify(
    {
      model: request.model,
      mode: "multi",
      rounds: 5,
      tool_calls: log.reduce((n, r) => n + r.tool_calls.length, 0),
      images: 1,
      unique_image_pixels: overview.width * overview.height,
      cumulative_image_pixels: overview.width * overview.height * 5,
      total_tokens: log.reduce((n, r) => n + r.usage.total_tokens, 0),
      retry_reason:
        "最终输出4500tokens截断，复用原请求扩至8000；包含失败轮费用",
      log,
    },
    null,
    2,
  ),
);
