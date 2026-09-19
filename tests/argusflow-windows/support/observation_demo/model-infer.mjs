/** 百炼盲推断入口：仅白名单证据文件；密钥只用于 HTTPS Authorization。 */
import { readFile, writeFile } from "node:fs/promises";
import { parseEnv } from "node:util";
import { resolve } from "node:path";
import { recordingSystemPrompt } from "./model-system.mjs";
import { saveResult } from "./prompts/result.mjs";
const directory = resolve(process.argv[2]);
const label = process.argv[3] ?? "model";
const system = recordingSystemPrompt;
if (!/^[a-z0-9-]+$/.test(label)) throw Error("Invalid output label");
const env = parseEnv(await readFile(resolve(".env"), "utf8"));
const key = env.alikey;
if (!key) throw Error(".env 缺少 alikey");
const model = "qwen3-vl-flash-2026-01-22";
const endpoint =
  "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
const evidencePath = process.argv[4] ?? "model-input/evidence.json";
if (
  ![
    "model-input/evidence.json",
    "model-input/normalized.json",
    "model-input/timeline.json",
  ].includes(evidencePath)
)
  throw Error("Only recorded evidence inputs are allowed");
const evidence = JSON.parse(
  await readFile(resolve(directory, evidencePath), "utf8"),
);
const frames = JSON.parse(
  await readFile(resolve(directory, "model-input/frames.json"), "utf8"),
);
const prompt = `根据真实录制证据和带编号截图重建操作，按系统中的 workflow 与 analysis 契约输出。保留请求与结果的区别；完整扫描最后事件。\n证据：\n${JSON.stringify(evidence)}`;
const content = [{ type: "text", text: prompt }];
for (const f of frames) {
  if (!/^model-input\/frame-\d+\.png$/.test(f.output))
    throw Error("Unexpected image path");
  const bytes = await readFile(resolve(directory, f.output));
  content.push(
    {
      type: "text",
      text: `截图 sample-${f.id}；原始裁剪范围 ${JSON.stringify(f.crop)}。`,
    },
    {
      type: "image_url",
      image_url: { url: `data:image/png;base64,${bytes.toString("base64")}` },
    },
  );
}
const messages = [
  ...(system ? [{ role: "system", content: system }] : []),
  { role: "user", content },
];
const body = {
  model,
  enable_thinking: false,
  temperature: 0,
  max_tokens: 6000,
  response_format: { type: "json_object" },
  messages,
};
await writeFile(
  resolve(directory, `${label}-request.json`),
  JSON.stringify(
    {
      endpoint,
      model,
      parameters: { temperature: 0, max_tokens: 6000, enable_thinking: false },
      system,
      prompt,
      frames,
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify({
    status: "requesting",
    model,
    images: frames.length,
    text_characters: prompt.length,
  }),
);
const started = Date.now();
const response = await fetch(endpoint, {
  method: "POST",
  headers: {
    Authorization: `Bearer ${key}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify(body),
  signal: AbortSignal.timeout(180000),
});
const result = await response.json();
// 即使服务异常回显，也禁止密钥落盘或输出。
const clean = JSON.stringify(result).split(key).join("[REDACTED]");
await writeFile(resolve(directory, `${label}-response.json`), clean);
if (!response.ok)
  throw Error(
    `Bailian HTTP ${response.status}; ${result.error?.code ?? result.code ?? "see redacted response"}`,
  );
if (result.choices?.[0]?.finish_reason !== "stop")
  throw Error("Model output incomplete");
const generated = await saveResult(
  directory,
  JSON.parse(result.choices[0].message.content),
  `${label}-`,
);
console.log(
  JSON.stringify({
    status: "complete",
    model: result.model,
    elapsed_ms: Date.now() - started,
    usage: result.usage,
    nodes:
      generated.workflow?.scopes.reduce((n, s) => n + s.nodes.length, 0) ?? 0,
    replay_ready: generated.analysis.replay_ready,
  }),
);
