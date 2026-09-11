import type { RunSnapshot } from "../../../features/workflow";
export type NodeRunState = "waiting" | "running" | "completed" | "failed";
/** 每个日志批次只扫描一次，递归画布共享状态表。 */
export function nodeStates(
  run: RunSnapshot | null,
  workflow: string,
): ReadonlyMap<string, NodeRunState> {
  const states = new Map<string, NodeRunState>();
  for (const item of run?.logs ?? []) {
    const location = item.path.at(-1);
    if (location?.workflow !== workflow || !location.node) continue;
    if (item.kind === "node_started") states.set(location.node, "running");
    else if (item.kind === "node_completed")
      states.set(location.node, "completed");
    else if (item.kind === "node_failed" || item.kind === "error")
      states.set(location.node, "failed");
  }
  return states;
}
