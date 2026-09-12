import { useMemo, useState } from "react";
import { Copy, Download, LocateFixed } from "lucide-react";
import {
  buildScene,
  nodeById,
  nodeTitle,
  studio,
  type RunLocation,
  type RunSnapshot,
} from "../../../features/workflow";
import { compose, inverse, fitBounds } from "../../../flow";
import { Button, Select } from "../../ui";

/** 日志定位切换文档和作用域，使用真实调用路径。 */
export async function locate(location: RunLocation): Promise<void> {
  if (location.workflow) await studio.open(location.workflow);
  const tab = studio.active;
  if (!tab) return;
  const scene = buildScene(tab.file),
    scope = scene.scopes[location.scope];
  if (!scope) return;
  const node = scope.nodes.find((node) => node.id === location.node);
  studio.view(
    compose(
      fitBounds(node ?? scope.bounds, 800, 500),
      inverse(scope.transform),
    ),
  );
  studio.select(location.node ? [location.node] : [], scope.id);
}
export const STATUS_LABELS = {
  running: "运行中",
  cleaning: "正在收尾",
  completed: "运行完成",
  failed: "运行失败",
  cancelled: "已停止",
  timed_out: "运行超时",
} as const;
export function LogPanel({ run }: { readonly run: RunSnapshot | null }) {
  const [filter, setFilter] = useState("all");
  const entries = useMemo(
    () =>
      run?.logs.filter((entry) => filter === "all" || entry.level === filter) ??
      [],
    [run, filter],
  );
  const text =
    run?.logs
      .map(
        (entry) =>
          "[" +
          entry.elapsed_ms +
          "ms] " +
          entry.level.toUpperCase() +
          " " +
          entry.path.map((item) => item.node ?? item.scope).join(" / ") +
          " " +
          entry.message,
      )
      .join("\n") ?? "";
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-line px-3">
        <span
          className={
            "mr-auto text-[11px] " +
            (run?.errors.length ? "text-danger" : "text-muted")
          }
        >
          {run
            ? STATUS_LABELS[run.status] +
              (run.omitted ? " · 已省略 " + run.omitted + " 条较早事件" : "")
            : "运行后在这里查看执行过程"}
        </span>
        <Select
          aria-label="日志级别"
          className="h-6 text-[10px]"
          value={filter}
          onValueChange={setFilter}
          options={[
            { value: "all", label: "全部" },
            { value: "error", label: "错误" },
            { value: "warning", label: "警告" },
          ]}
        />
        <Button
          variant="ghost"
          aria-label="复制日志"
          disabled={!run}
          className="h-6 px-1.5"
          onClick={() => {
            void studio.safely(() => studio.api.copy(text));
          }}
        >
          <Copy size={12} />
        </Button>
        <Button
          variant="ghost"
          aria-label="导出日志"
          disabled={!run}
          className="h-6 px-1.5"
          onClick={() => {
            void studio.safely(() => studio.api.exportLog(text));
          }}
        >
          <Download size={12} />
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto font-mono text-[11px]">
        {entries.map((entry) => {
          const location = entry.path.at(-1);
          const tab = location?.workflow
            ? studio.store.getState().tabs[location.workflow]
            : undefined;
          const node =
            tab && location?.node
              ? nodeById(tab.file, location.node)
              : undefined;
          const title =
            tab && node
              ? nodeTitle(tab.file, node)
              : (location?.node ?? "工作流");
          return (
            <div
              key={entry.sequence}
              className={
                "group grid min-h-7 grid-cols-[64px_50px_140px_minmax(0,1fr)_28px] items-start gap-2 border-b border-line/50 px-3 py-1 " +
                (entry.level === "error"
                  ? "bg-danger-soft text-danger"
                  : "text-muted")
              }
            >
              <span className="tabular-nums">
                {(Number(entry.elapsed_ms) / 1000).toFixed(2)}s
              </span>
              <span className="uppercase">{entry.level}</span>
              <span className="truncate font-sans" title={title}>
                {title}
              </span>
              <span className="whitespace-pre-wrap break-words font-sans">
                {entry.message}
              </span>
              {location?.node && (
                <Button
                  variant="ghost"
                  aria-label="定位节点"
                  className="h-5 px-1 opacity-0 group-hover:opacity-100 focus:opacity-100"
                  onClick={() => {
                    void studio.safely(() => locate(location));
                  }}
                >
                  <LocateFixed size={12} />
                </Button>
              )}
            </div>
          );
        })}
        {!entries.length && (
          <p className="p-4 font-sans text-muted">
            {run
              ? "没有符合筛选条件的日志。"
              : "尚未运行。配置完成后点击右上角“运行”。"}
          </p>
        )}
      </div>
    </div>
  );
}
