import { isTauri } from "@tauri-apps/api/core";
import type { ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { studio } from "../../features/workflow";
import { Button } from "../ui";
/** 标题栏承载文档导航、窗口拖动与原生窗口操作。 */
export function TitleBar({ tabs }: { readonly tabs: ReactNode }) {
  const windowAction = (action: "minimize" | "toggleMaximize" | "close") => {
    if (isTauri()) void studio.safely(() => getCurrentWindow()[action]());
  };
  return (
    <header className="flex h-10 shrink-0 select-none items-center border-b border-line bg-panel pl-3">
      <img
        src="/icon.png"
        alt=""
        className="mr-2 size-6 shrink-0 object-contain"
      />
      <span className="shrink-0 text-sm font-semibold tracking-tight">
        ArgusFlow
      </span>
      {tabs}
      <div
        className="h-full min-w-20 flex-1"
        data-tauri-drag-region
        onDoubleClick={() => windowAction("toggleMaximize")}
      />
      <Button
        variant="ghost"
        aria-label="最小化"
        className="h-10 w-10 rounded-none px-0"
        onClick={() => windowAction("minimize")}
      >
        <Minus size={14} />
      </Button>
      <Button
        variant="ghost"
        aria-label="最大化或还原"
        className="h-10 w-10 rounded-none px-0"
        onClick={() => windowAction("toggleMaximize")}
      >
        <Square size={11} />
      </Button>
      <Button
        variant="ghost"
        aria-label="关闭窗口"
        className="h-10 w-11 rounded-none px-0 text-ink hover:bg-[#c42b1c] hover:text-white active:bg-[#a82418] active:text-white"
        onClick={() => windowAction("close")}
      >
        <X size={15} />
      </Button>
    </header>
  );
}
