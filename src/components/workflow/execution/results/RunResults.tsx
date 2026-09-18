import { useState } from "react";
import { useStore } from "zustand";
import {
  CheckCircle2,
  CircleAlert,
  Clipboard,
  Inbox,
  LoaderCircle,
} from "lucide-react";
import {
  studio,
  type RunSnapshot,
  type RunStatus,
} from "../../../../features/workflow";
import { Button } from "../../../ui";
import { ResultContent } from "./ResultContent";
import { resultClipboard, resultSummary } from "./valuePresentation";

/** 运行状态仅描述本次执行，失败时仍允许查看已产生的结果。 */
const STATUS_LABELS: Readonly<Record<RunStatus, string>> = {
  running: "正在运行",
  cleaning: "正在结束",
  completed: "运行完成",
  failed: "运行失败",
  cancelled: "已停止",
  timed_out: "运行超时",
};

/** 独立只读结果面板，绝不把运行值混进流程设置。 */
export function RunResults({ workflow }: { readonly workflow: string }) {
  const run = useStore(studio.store, (state) => state.run);
  if (!run || run.workflow !== workflow)
    return (
      <section
        aria-label="运行结果"
        className="flex h-full flex-col items-center justify-center gap-3 text-muted"
      >
        <Inbox size={32} strokeWidth={1.4} />
        <h3 className="text-sm font-medium text-ink">还没有运行结果</h3>
        <p className="text-sm">运行此流程后，在这里查看结果。</p>
      </section>
    );
  return <Results key={run.id} run={run} />;
}

function Results({ run }: { readonly run: RunSnapshot }) {
  const [selected, setSelected] = useState<string>();
  const entries = Object.entries(run.outputs);
  const current = entries.find(([name]) => name === selected) ?? entries[0];
  const active = run.status === "running" || run.status === "cleaning";
  const Icon = active
    ? LoaderCircle
    : run.status === "completed"
      ? CheckCircle2
      : CircleAlert;
  return (
    <section aria-label="运行结果" className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 items-center gap-2 border-b border-line px-4 py-3">
        <Icon
          size={16}
          className={
            active
              ? "animate-spin text-accent"
              : run.status === "completed"
                ? "text-accent"
                : "text-danger"
          }
        />
        <h3 className="text-sm font-medium">{STATUS_LABELS[run.status]}</h3>
        <span className="text-xs text-muted">{entries.length} 项结果</span>
        {run.errors.length > 0 && (
          <Button
            variant="ghost"
            className="ml-auto"
            onClick={() => studio.panel("logs")}
          >
            查看失败原因
          </Button>
        )}
      </header>
      {!current ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
          <Inbox size={32} className="text-muted" strokeWidth={1.4} />
          <p className="text-sm">
            {active ? "结果将在运行结束后显示" : "本次运行没有返回内容"}
          </p>
          {!active && (
            <Button variant="ghost" onClick={() => studio.panel("data")}>
              打开流程设置
            </Button>
          )}
        </div>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-[minmax(140px,220px)_minmax(0,1fr)]">
          <nav
            aria-label="结果列表"
            className="space-y-1 overflow-y-auto border-r border-line bg-subtle/30 p-2 [scrollbar-width:thin]"
          >
            {entries.map(([name, value]) => (
              <Button
                key={name}
                variant="ghost"
                aria-pressed={name === current[0]}
                className={
                  "h-auto w-full justify-start px-3 py-3 text-left " +
                  (name === current[0] ? "bg-accent-soft text-accent" : "")
                }
                onClick={() => setSelected(name)}
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium">
                    {name}
                  </span>
                  <span className="mt-1 block truncate text-xs font-normal text-muted">
                    {resultSummary(value)}
                  </span>
                </span>
              </Button>
            ))}
          </nav>
          <div className="min-w-0 overflow-auto p-5 [scrollbar-width:thin]">
            <div className="mb-5 flex items-center justify-between gap-4">
              <h4 className="min-w-0 break-words text-base font-semibold">
                {current[0]}
              </h4>
              <Button
                variant="ghost"
                onClick={() =>
                  void studio.safely(() =>
                    studio.api.copy(resultClipboard(current[1])),
                  )
                }
              >
                <Clipboard size={14} />
                复制内容
              </Button>
            </div>
            <ResultContent value={current[1]} />
          </div>
        </div>
      )}
    </section>
  );
}
