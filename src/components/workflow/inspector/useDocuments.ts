import { useStore } from "zustand";
import { studio } from "../../../features/workflow";
/** 未打开的调用目标也提供端口类型；已打开的草稿优先。 */
export function useDocuments() {
  const tabs = useStore(studio.store, (state) => state.tabs);
  const references = useStore(studio.store, (state) => state.references);
  return Object.values({
    ...references,
    ...Object.fromEntries(
      Object.entries(tabs).map(([id, tab]) => [id, tab.file]),
    ),
  });
}
