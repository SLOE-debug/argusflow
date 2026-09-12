import type { StoreApi } from "zustand/vanilla";
import type { DesktopApi, LoadedDocument } from "../api/desktop";
import type { DocumentPersistence } from "./persistence";
import type { StudioState } from "./state";

interface ListedDocument {
  readonly id: string;
  readonly name: string;
  readonly revision: string | null;
}

/** 列表包含尚在自动保存的新流程，名称始终以编辑中的文档为准。 */
export function listedDocuments(state: StudioState) {
  const documents = new Map<string, ListedDocument>(
    state.documents.map((item) => [item.id, item]),
  );
  for (const tab of Object.values(state.tabs))
    documents.set(tab.file.id, {
      id: tab.file.id,
      name: tab.file.definition.name,
      revision: tab.revision,
    });
  return [...documents.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function documentReadonly(state: StudioState, id: string): boolean {
  return (
    state.busy ||
    Boolean(
      state.run &&
      ["running", "cleaning"].includes(state.run.status) &&
      state.run.documents.includes(id),
    )
  );
}

/** 文档级操作针对明确身份，不依赖当前激活标签；与自动保存串行。 */
export class DocumentLibrary {
  constructor(
    private readonly store: StoreApi<StudioState>,
    private readonly api: DesktopApi,
    private readonly persistence: DocumentPersistence,
  ) {}

  async rename(id: string, name: string): Promise<void> {
    const trimmed = name.trim();
    if (!trimmed) throw new Error("请输入工作流名称");
    await this.change(id, async () => {
      const tab = this.store.getState().tabs[id];
      if (tab) {
        if (tab.file.definition.name !== trimmed) {
          const file = {
            ...tab.file,
            definition: { ...tab.file.definition, name: trimmed },
          };
          this.store.setState({
            tabs: {
              ...this.store.getState().tabs,
              [id]: {
                ...tab,
                file,
                version: tab.version + 1,
                status: tab.status === "conflict" ? "conflict" : "dirty",
                past: [...tab.past.slice(-99), tab.file],
                future: [],
              },
            },
          });
        }
        await this.persistence.flush(id);
        const saved = this.store.getState().tabs[id];
        if (saved?.revision)
          this.publish({ file: saved.file, revision: saved.revision });
      } else {
        const loaded = await this.api.load(id);
        const saved = await this.api.save(
          {
            ...loaded.file,
            definition: { ...loaded.file.definition, name: trimmed },
          },
          loaded.revision,
        );
        this.publish(saved);
      }
    });
  }

  async remove(id: string): Promise<void> {
    await this.change(id, async () => {
      // 等待已启动的保存和待保存编辑落盘，删除后不再有保存请求复建文件。
      await this.persistence.flush(id);
      const state = this.store.getState();
      const revision =
        state.tabs[id]?.revision ??
        state.documents.find((item) => item.id === id)?.revision;
      if (!revision) throw new Error("工作流尚未保存，请稍后重试");
      await this.api.deleteDocument(id, revision);
      const latest = this.store.getState();
      const tabs = { ...latest.tabs },
        references = { ...latest.references };
      delete tabs[id];
      delete references[id];
      this.store.setState({
        tabs,
        references,
        documents: latest.documents.filter((item) => item.id !== id),
        active:
          latest.active === id
            ? (Object.keys(tabs).at(-1) ?? null)
            : latest.active,
        ...(latest.active === id ? { problems: [], aqlNode: null } : {}),
      });
    });
  }

  private publish(saved: LoadedDocument): void {
    const state = this.store.getState();
    this.store.setState({
      documents: [
        ...state.documents.filter((item) => item.id !== saved.file.id),
        {
          id: saved.file.id,
          name: saved.file.definition.name,
          revision: saved.revision,
        },
      ].sort((a, b) => a.name.localeCompare(b.name)),
      references: { ...state.references, [saved.file.id]: saved.file },
    });
  }
  private async change(id: string, action: () => Promise<void>): Promise<void> {
    if (documentReadonly(this.store.getState(), id))
      throw new Error("工作流正在使用中，请稍后重试");
    this.store.setState({ busy: true });
    try {
      await action();
    } finally {
      this.store.setState({ busy: false });
    }
  }
}
