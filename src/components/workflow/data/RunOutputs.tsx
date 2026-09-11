import { useStore } from "zustand";
import { studio } from "../../../features/workflow";
import { Button } from "../../ui";
/** 输出仅在结果区域显式查看，不混入逐节点日志。 */
export function RunOutputs({ workflow }: { readonly workflow: string }) {
  const run = useStore(studio.store, (state) => state.run);
  if (
    run?.workflow !== workflow ||
    !["completed", "failed", "cancelled", "timed_out"].includes(run.status)
  )
    return null;
  const source = JSON.stringify(run.outputs, null, 2);
  return (
    <details
      className="mt-4 border-t border-line pt-3"
      open={run.status === "completed"}
    >
      <summary className="cursor-pointer text-xs text-muted">
        最近一次运行结果
      </summary>
      <pre className="mt-2 max-h-48 overflow-auto rounded-md bg-subtle p-3 text-[11px] text-ink">
        {source}
      </pre>
      <Button
        variant="ghost"
        className="mt-1 h-6 px-0 text-[11px]"
        onClick={() => {
          void studio.safely(() => studio.api.copy(source));
        }}
      >
        复制结果
      </Button>
    </details>
  );
}
