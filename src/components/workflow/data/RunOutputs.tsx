import { Collapse } from "../../ui";
import { useStore } from "zustand";
import {
  studio,
  isSpatialPreview,
  parseSpatialPreview,
} from "../../../features/workflow";
import { Button } from "../../ui";
import { SpatialPreview } from "./SpatialPreview";
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
    <Collapse
      className="mt-4 border-t border-line pt-3"
      open={run.status === "completed"}
      title={<>最近一次运行结果</>}
    >
      {Object.entries(run.outputs)
        .filter(([, value]) => isSpatialPreview(value))
        .map(([name, value]) => {
          const previews = parseSpatialPreview(value);
          return previews ? (
            <SpatialPreview key={name} previews={previews} />
          ) : (
            <p key={name} role="alert">
              空间预览数据无效：{name}
            </p>
          );
        })}
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
    </Collapse>
  );
}
