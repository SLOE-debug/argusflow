import { useEffect, useRef } from "react";
import { useStore } from "zustand";
import { X } from "lucide-react";
import { studio } from "../../../features/workflow";
import { Button, IconButton } from "../../ui";

/** 标题栏内的文档导航；横向滚动范围不包含窗口控制和拖动区。 */
export function WorkflowTabs() {
  const state = useStore(studio.store);
  const list = useRef<HTMLDivElement>(null);
  const tabs = Object.values(state.tabs);
  useEffect(() => {
    list.current
      ?.querySelector<HTMLElement>('[aria-selected="true"]')
      ?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [state.active]);
  if (!tabs.length) return null;
  return (
    <div className="ml-2 flex min-w-0 shrink items-center self-stretch">
      <div
        ref={list}
        role="tablist"
        aria-label="工作流标签"
        className="flex min-w-0 items-center overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        onKeyDown={(event) => {
          if (
            !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) ||
            !tabs.length
          )
            return;
          if (
            !(event.target instanceof HTMLElement) ||
            event.target.getAttribute("role") !== "tab"
          )
            return;
          event.preventDefault();
          const index = tabs.findIndex((item) => item.file.id === state.active);
          const next =
            event.key === "Home"
              ? 0
              : event.key === "End"
                ? tabs.length - 1
                : (index +
                    (event.key === "ArrowRight" ? 1 : -1) +
                    tabs.length) %
                  tabs.length;
          void studio.safely(() => studio.open(tabs[next].file.id));
          list.current
            ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
            [next]?.focus();
        }}
      >
        {tabs.map((item) => (
          <div
            key={item.file.id}
            className={
              "group mr-1 flex h-6 min-w-24 max-w-48 shrink-0 items-center rounded-md border hover:bg-hover has-[:focus-visible]:bg-hover " +
              (state.active === item.file.id
                ? "border-line bg-surface shadow-xs"
                : "border-transparent")
            }
          >
            <Button
              role="tab"
              aria-selected={state.active === item.file.id}
              tabIndex={state.active === item.file.id ? 0 : -1}
              title={item.file.definition.name}
              variant="ghost"
              className={
                "h-full min-w-0 flex-1 justify-start gap-2 border-0 px-2 py-0 text-xs font-medium leading-4 hover:bg-transparent focus-visible:bg-transparent " +
                (state.active === item.file.id ? "text-ink" : "text-muted")
              }
              onClick={() => {
                void studio.safely(() => studio.open(item.file.id));
              }}
            >
              <span className="block truncate leading-4">
                {item.file.definition.name}
              </span>
              {item.version !== item.savedVersion && (
                <span
                  aria-label="未保存"
                  className="size-1.5 shrink-0 rounded-full bg-accent"
                />
              )}
            </Button>
            <IconButton
              aria-label={"关闭 " + item.file.definition.name}
              className="mr-0.5 size-5 text-muted hover:bg-transparent focus-visible:bg-transparent group-hover:text-ink focus-visible:text-ink"
              onClick={() => {
                void studio.safely(() => studio.close(item.file.id));
              }}
            >
              <X size={11} />
            </IconButton>
          </div>
        ))}
      </div>
    </div>
  );
}
