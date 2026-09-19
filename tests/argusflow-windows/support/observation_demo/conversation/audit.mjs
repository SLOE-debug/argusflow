/** 以原始按键和真实选区做外部审计，不将标准答案回传模型。 */
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { loadEvidence } from "./evidence.mjs";
const [directory, ...names] = process.argv.slice(2);
const recording = await loadEvidence(resolve(directory));
const expected = [];
for (const item of recording.timeline) {
  if (item.kind === "copy")
    expected.push({
      id: item.id,
      action: "copy",
      app: "Chrome",
      text: item.selection.text,
    });
  if (
    item.kind !== "keyboard_chord" ||
    !item.keys.some((k) => k.endsWith("Control"))
  )
    continue;
  const action = { C: "copy", V: "paste", S: "save_request" }[item.keys.at(-1)];
  if (!action) continue;
  const before = recording.source.samples.filter(
    (s) => s.through_epoch_ms <= item.ms + recording.origin,
  );
  const state = before.findLast((s) => s.window.class === "Notepad")?.focus
    ?.text?.Available;
  const clipboard = before.findLast(
    (s) => s.clipboard_current !== null,
  )?.clipboard_current;
  const selected = state?.selections.find((s) => !s.collapsed);
  expected.push({
    id: item.id,
    action,
    app: "Notepad",
    text:
      action === "copy"
        ? selected?.text
        : action === "paste"
          ? clipboard
          : null,
    ...(action === "copy" && selected
      ? {
          line: state.document
            .slice(0, selected.start_utf16)
            .split(/\r\n|\r|\n/).length,
          range: [selected.start_utf16, selected.end_utf16],
        }
      : {}),
  });
}
const output = [];
for (const name of names) {
  if (!/^[a-z0-9-]+$/.test(name)) throw Error("未知实验目录");
  const workflow = JSON.parse(
    await readFile(resolve(directory, name, "workflow.json"), "utf8"),
  );
  const metrics = JSON.parse(
    await readFile(resolve(directory, name, "metrics.json"), "utf8"),
  );
  const steps = workflow.steps.filter((s) =>
    ["copy", "paste", "save_request"].includes(s.action),
  );
  const mismatches = [];
  for (let i = 0; i < Math.max(steps.length, expected.length); i++) {
    const actual = steps[i],
      target = expected[i];
    if (
      !actual ||
      !target ||
      actual.action !== target.action ||
      (target.text !== null && actual.text !== target.text) ||
      (target.line && actual.line !== target.line)
    )
      mismatches.push({ position: i + 1, expected: target, actual });
  }
  const refs = workflow.steps.flatMap((step, index) =>
    step.evidence_ids
      .filter((id) => !recording.details.has(String(id)))
      .map((id) => ({ step: index + 1, id })),
  );
  const copyRanges = workflow.steps
    .filter((s) => s.action === "copy" && /notepad/i.test(s.source_app ?? ""))
    .map((s) => ({ text: s.text, line: s.line, range: s.selection_range }));
  output.push({
    name,
    model: metrics.model,
    core_actions_expected: expected.length,
    core_actions_returned: steps.length,
    sequence_text_line_mismatches: mismatches,
    invalid_references: refs,
    native_copy_ranges: copyRanges,
    replay_ready: workflow.replay_ready,
    total_tokens: metrics.total_tokens,
    images: metrics.images,
    rounds: metrics.rounds,
    tools: metrics.tool_calls,
    prompt_tokens: metrics.log.reduce((n, r) => n + r.usage.prompt_tokens, 0),
    completion_tokens: metrics.log.reduce(
      (n, r) => n + r.usage.completion_tokens,
      0,
    ),
  });
}
await writeFile(
  resolve(directory, "conversation-audit.json"),
  JSON.stringify(output, null, 2),
);
console.log(
  JSON.stringify(
    output.map((x) => ({
      ...x,
      sequence_text_line_mismatches: x.sequence_text_line_mismatches.length,
      invalid_references: x.invalid_references.length,
    })),
    null,
    2,
  ),
);
