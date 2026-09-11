import { useStore } from "zustand";
import { Plus, RefreshCw, Workflow } from "lucide-react";
import { studio } from "../../../features/workflow";
import { Button, IconButton } from "../../ui";

/** 工作流列表只展示固定数据目录的内容，不提供目录选择。 */
export function DocumentList({ query }: { readonly query: string }) {
  const state = useStore(studio.store);
  const ready = state.initialization.status === "ready";
  const documents = state.documents.filter((item) =>
    item.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  return (
    <>
      <div className="flex h-7 shrink-0 items-center gap-1 px-2">
        <span className="mr-auto text-[11px] text-muted">
          工作流 · {state.documents.length}
        </span>
        <IconButton
          aria-label="新建流程"
          disabled={!ready}
          onClick={() => {
            void studio.safely(() => studio.create());
          }}
        >
          <Plus size={13} />
        </IconButton>
        <IconButton
          aria-label="刷新工作流"
          disabled={!ready}
          onClick={() => {
            void studio.safely(() => studio.refreshDocuments());
          }}
        >
          <RefreshCw size={12} />
        </IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {documents.map((item) => (
          <Button
            key={item.id}
            variant="ghost"
            className={
              "h-7 w-full justify-start gap-2 px-2 text-xs " +
              (state.active === item.id
                ? "bg-accent-soft text-accent"
                : "text-ink")
            }
            onClick={() => {
              void studio.safely(() => studio.open(item.id));
            }}
          >
            <Workflow size={13} className="shrink-0" />
            <span className="truncate">{item.name}</span>
          </Button>
        ))}
        {!documents.length && (
          <p className="px-2 py-4 text-xs leading-5 text-muted">
            {query
              ? "没有匹配的工作流"
              : ready
                ? "还没有工作流，点击上方 ＋ 新建。"
                : "正在准备工作流数据…"}
          </p>
        )}
      </div>
    </>
  );
}
