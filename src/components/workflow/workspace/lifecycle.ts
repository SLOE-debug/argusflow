import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { studio } from "../../../features/workflow";

let initialized: Promise<void> | undefined;
let closing = false;
/** 生命周期独立于 React 重挂载；关闭保存失败时保留窗口和草稿。 */
export function initializeDesktop(): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  initialized ??= (async () => {
    await listen("desktop-close-requested", () => {
      if (closing) return;
      closing = true;
      void studio.safely(async () => {
        try {
          studio.message("正在保存并结束活动运行…");
          await studio.flushAll();
          await invoke("shutdown_desktop");
        } finally {
          closing = false;
        }
      });
    });
    await getCurrentWindow().show();
    // 运行订阅故障不应阻止独立的文档初始化和重试。
    await studio.safely(() => studio.subscribe());
    await studio.initializeWorkspace();
  })();
  return initialized;
}
