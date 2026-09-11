import {
  Play,
  Square,
  Check,
  ShieldCheck,
  Save,
  Undo2,
  Redo2,
} from "lucide-react";
import { useStore } from "zustand";
import { studio, type EditorTab } from "../../../features/workflow";
import { Button } from "../../ui";
import { STATUS_LABELS } from "../execution/LogPanel";
const SAVE_LABELS = {
  saved: "已保存",
  dirty: "待保存",
  saving: "保存中…",
  failed: "保存失败",
  conflict: "文件冲突",
} as const;
export function Toolbar({
  tab,
  onRun,
}: {
  readonly tab: EditorTab;
  readonly onRun: () => void;
}) {
  const state = useStore(studio.store);
  const running =
    state.run && ["running", "cleaning"].includes(state.run.status);
  return (
    <div
      role="toolbar"
      aria-label="工作流操作"
      className="flex h-10 shrink-0 items-center gap-1.5 border-b border-line bg-panel px-3 [&>button]:h-7"
    >
      {tab && (
        <>
          <Button
            variant="ghost"
            title="撤销 Ctrl+Z"
            aria-label="撤销"
            className="px-1.5"
            disabled={!tab.past.length || studio.readonly}
            onClick={() => studio.undo()}
          >
            <Undo2 size={14} />
          </Button>
          <Button
            variant="ghost"
            title="重做 Ctrl+Y"
            aria-label="重做"
            className="px-1.5"
            disabled={!tab.future.length || studio.readonly}
            onClick={() => studio.redo()}
          >
            <Redo2 size={14} />
          </Button>
        </>
      )}
      {tab && (
        <Button
          variant="ghost"
          className={
            "text-[11px] " +
            (["failed", "conflict"].includes(tab.status)
              ? "text-danger"
              : "text-muted")
          }
          title={tab.error}
          onClick={() => {
            void studio.safely(() => studio.flushAll());
          }}
        >
          {tab.status === "saved" ? (
            <Check size={12} className="text-success" />
          ) : (
            <Save size={12} />
          )}
          {SAVE_LABELS[tab.status]}
        </Button>
      )}
      <span className="flex-1" />
      {running && (
        <span className="mr-2 text-[11px] text-accent">
          {STATUS_LABELS[state.run!.status]}
        </span>
      )}
      <Button
        disabled={!tab || studio.readonly}
        onClick={() => {
          void studio.safely(() => studio.validate());
        }}
      >
        <ShieldCheck size={13} />
        校验
      </Button>
      {running ? (
        <Button
          variant="danger"
          onClick={() => {
            void studio.safely(() => studio.stop());
          }}
        >
          <Square size={12} />
          停止
        </Button>
      ) : (
        <Button variant="primary" disabled={!tab || state.busy} onClick={onRun}>
          <Play size={13} fill="currentColor" />
          运行
        </Button>
      )}
    </div>
  );
}
