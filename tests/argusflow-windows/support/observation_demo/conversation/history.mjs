/** 从审计记录恢复只有一张开场图的原对话；不悄悄丢弃图像。 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
export async function restoreOverviewConversation(directory, round) {
  const request = JSON.parse(
    await readFile(resolve(directory, `request-${round}.json`), "utf8"),
  );
  let count = 0;
  for (const message of request.messages)
    if (Array.isArray(message.content))
      for (const part of message.content) {
        if (part.type !== "image_url") continue;
        if (
          ++count > 1 ||
          part.image_url.url !== "[local image payload omitted]"
        )
          throw Error("此恢复入口仅支持一张开场图");
        part.image_url.url = `data:image/png;base64,${(await readFile(resolve(directory, "images/overview-0.png"))).toString("base64")}`;
      }
  return request;
}
