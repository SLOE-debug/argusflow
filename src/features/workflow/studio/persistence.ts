import type { StoreApi } from "zustand/vanilla";
import type { DesktopApi } from "../api/desktop";
import type { EditorTab, StudioState } from "./state";

/** 每份文档只有一个保存任务，编辑版本和磁盘版本分开追踪。 */
export class DocumentPersistence {
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private readonly flights = new Map<string, Promise<void>>();
  constructor(
    private readonly store: StoreApi<StudioState>,
    private readonly api: DesktopApi,
  ) {}
  schedule(id: string): void {
    clearTimeout(this.timers.get(id));
    this.timers.set(
      id,
      setTimeout(() => {
        this.timers.delete(id);
        void this.flush(id).catch(() => {});
      }, 800),
    );
  }
  async flush(id: string): Promise<void> {
    clearTimeout(this.timers.get(id));
    this.timers.delete(id);
    const previous = this.flights.get(id);
    if (previous) {
      await previous;
      return this.flush(id);
    }
    const tab = this.store.getState().tabs[id];
    if (!tab || tab.version === tab.savedVersion) return;
    if (tab.status === "conflict") throw new Error(tab.error ?? "文件发生冲突");
    const operation = this.save(id, tab);
    this.flights.set(id, operation);
    try {
      await operation;
    } finally {
      if (this.flights.get(id) === operation) this.flights.delete(id);
    }
    const latest = this.store.getState().tabs[id];
    if (latest && latest.version !== latest.savedVersion) await this.flush(id);
  }
  private async save(id: string, tab: EditorTab): Promise<void> {
    this.update(id, (current) => ({
      ...current,
      status: "saving",
      error: undefined,
    }));
    try {
      const saved = await this.api.save(tab.file, tab.revision);
      this.update(id, (current) => ({
        ...current,
        revision: saved.revision,
        savedVersion: tab.version,
        status: current.version === tab.version ? "saved" : "dirty",
      }));
      const state = this.store.getState();
      const summary = {
        id,
        name: saved.file.definition.name,
        revision: saved.revision,
      };
      this.store.setState({
        documents: [
          ...state.documents.filter((item) => item.id !== id),
          summary,
        ].sort((a, b) => a.name.localeCompare(b.name)),
      });
    } catch (error) {
      const message = String(error);
      this.update(id, (current) => ({
        ...current,
        status: message.includes("conflict:") ? "conflict" : "failed",
        error: message,
      }));
      this.store.setState({ message });
      throw error;
    }
  }
  private update(id: string, update: (tab: EditorTab) => EditorTab): void {
    const tabs = this.store.getState().tabs;
    if (tabs[id])
      this.store.setState({ tabs: { ...tabs, [id]: update(tabs[id]) } });
  }
}
