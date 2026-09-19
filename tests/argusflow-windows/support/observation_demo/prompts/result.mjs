/** 保存审计旁路文件；只有通过正式 Rust 编译契约的文档才命名为 workflow.json。 */
import { spawn } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("../../../../../", import.meta.url));

/** 只调用 prepare，不执行桌面操作。首次调用由 Cargo 构建验证器。 */
export function validateContract(workflow, queries = []) {
  return new Promise((accept, reject) => {
    const child = spawn(
      "cargo",
      [
        "run",
        "--quiet",
        "-p",
        "argusflow-workflow-automation",
        "--example",
        "validate_recording_workflow",
      ],
      { cwd: root, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] },
    );
    let stderr = "",
      stdout = "";
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("workflow 契约验证超时"));
    }, 120000);
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr = (stderr + chunk).slice(-12000);
    });
    child.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (code === 0) accept(JSON.parse(stdout));
      else reject(new Error(`workflow 契约验证失败：${stderr}`));
    });
    child.stdin.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.stdin.end(JSON.stringify({ workflow, queries }));
  });
}

/** 审计与可执行文档严格分开，旧 steps 格式直接拒绝。 */
export async function saveResult(output, result, prefix = "") {
  if (!result || Object.keys(result).sort().join(",") !== "analysis,workflow")
    throw new Error("模型响应必须仅含 workflow 和 analysis");
  const analysis = result.analysis;
  if (
    !analysis ||
    analysis.replay_ready !== false ||
    typeof analysis.summary !== "string" ||
    !["node_evidence", "unresolved", "required_bindings"].every((key) =>
      Array.isArray(analysis[key]),
    )
  )
    throw new Error("模型审计契约无效");
  if (result.workflow === null && analysis.unresolved.length === 0)
    throw new Error("workflow 为 null 时必须说明缺失能力或证据");
  if (result.workflow !== null && analysis.unresolved.length)
    throw new Error("存在未解决操作时不能输出完整 workflow");
  await writeFile(
    resolve(output, `${prefix}model-result.json`),
    JSON.stringify(result, null, 2),
  );
  await writeFile(
    resolve(output, `${prefix}analysis.json`),
    JSON.stringify(analysis, null, 2),
  );
  if (result.workflow !== null) {
    const validation = await validateContract(result.workflow);
    await writeFile(
      resolve(output, `${prefix}validation.json`),
      JSON.stringify(validation, null, 2),
    );
    await writeFile(
      resolve(output, `${prefix}workflow.json`),
      JSON.stringify(result.workflow, null, 2),
    );
  }
  return result;
}
