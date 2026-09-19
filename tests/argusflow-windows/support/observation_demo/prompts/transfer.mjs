/** 历史 demo 与正式 AI 模块共享同一引擎契约。 */
import { readFileSync } from "node:fs";
export const transferPrompt = readFileSync(new URL("../../../../../crates/argusflow-ai/src/inference/prompts/transfer.txt", import.meta.url), "utf8");
