import { useEffect, useId, useRef, useState } from "react";
import { Check, History } from "lucide-react";
import { Button } from "../ui";
import type { useRecorder } from "../../features/recorder";

/** 历史列表浮在回看区域上方，保持在原生对话框的焦点范围内。 */
export function RecorderHistoryPopover({
  recorder,
}: {
  readonly recorder: ReturnType<typeof useRecorder>;
}) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target))
        setOpen(false);
    };
    window.addEventListener("pointerdown", dismiss);
    return () => window.removeEventListener("pointerdown", dismiss);
  }, [open]);
  return (
    <div
      ref={root}
      className="relative shrink-0"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
      onKeyDown={(event) => {
        if (open && event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <Button
        ref={trigger}
        variant="ghost"
        className="h-7 px-2"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen((value) => !value)}
      >
        <History size={14} />
        录制历史
      </Button>
      {open && (
        <div
          id={id}
          role="region"
          aria-label="录制历史"
          className="absolute right-0 top-full z-20 mt-1 max-h-[min(20rem,60vh)] w-64 max-w-[calc(100vw-4rem)] overflow-y-auto rounded-md border border-line bg-surface p-1 text-xs shadow-lg"
        >
          {recorder.entries.map((entry) => (
            <Button
              key={entry.directory}
              variant="ghost"
              aria-pressed={recorder.opened?.directory === entry.directory}
              disabled={recorder.busy}
              className="h-auto min-h-8 w-full justify-between gap-2 rounded-sm px-2 py-1.5 text-left"
              onClick={() =>
                void recorder.safely(async () => {
                  await recorder.open(entry.directory);
                  setOpen(false);
                  trigger.current?.focus();
                })
              }
            >
              <span>{new Date(entry.session.created_ms).toLocaleString()}</span>
              {recorder.opened?.directory === entry.directory && (
                <Check size={14} className="shrink-0" />
              )}
            </Button>
          ))}
          {!recorder.entries.length && (
            <p className="px-2 py-3 text-muted">暂无录制</p>
          )}
        </div>
      )}
    </div>
  );
}
