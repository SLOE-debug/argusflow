/** 百炼多轮传输；请求审计去除 base64，密钥不进入日志与产物。 */
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { parseEnv } from "node:util";
const endpoint =
  "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

export async function makeClient(output, model, textBudget = 90_000) {
  if (!Number.isInteger(textBudget) || textBudget < 1 || textBudget > 180_000)
    throw Error("文本预算须在 1–180000 字符之间");
  const key = parseEnv(await readFile(resolve(".env"), "utf8")).alikey;
  if (!key) throw Error("缺少 alikey");
  return async (messages, tools, round, maxTokens = 4500) => {
    if (!Number.isInteger(maxTokens) || maxTokens < 1 || maxTokens > 16000)
      throw Error("输出 token 预算无效");
    const body = {
      model,
      enable_thinking: false,
      temperature: 0,
      max_tokens: maxTokens,
      ...(!tools.length ? { response_format: { type: "json_object" } } : {}),
      messages,
      ...(tools.length ? { tools, tool_choice: "auto" } : {}),
    };
    const manifest = JSON.stringify(body, (name, value) =>
      name === "url" &&
      typeof value === "string" &&
      value.startsWith("data:image/")
        ? "[local image payload omitted]"
        : value,
    );
    if (manifest.length > textBudget)
      throw Error(`对话文本 ${manifest.length} 超过 ${textBudget} 字符预算`);
    await writeFile(resolve(output, `request-${round}.json`), manifest);
    const started = Date.now();
    const response = await fetch(endpoint, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${key}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(240000),
    });
    const raw = await response.text();
    const sanitized = raw.split(key).join("[REDACTED]");
    await writeFile(resolve(output, `response-${round}.json`), sanitized);
    if (!response.ok)
      throw Error(`百炼 HTTP ${response.status}，已保存脱敏响应`);
    const result = JSON.parse(sanitized);
    if (!["stop", "tool_calls"].includes(result.choices?.[0]?.finish_reason))
      throw Error("模型响应未完整结束");
    console.log(
      JSON.stringify({
        round,
        elapsed_ms: Date.now() - started,
        usage: result.usage,
        tools:
          result.choices[0].message.tool_calls?.map((t) => t.function.name) ??
          [],
      }),
    );
    return result;
  };
}
