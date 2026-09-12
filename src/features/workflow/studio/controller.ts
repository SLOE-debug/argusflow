import { createStore } from "zustand/vanilla";
import type { FlowPoint, ViewportTransform } from "../../../flow";
import { desktopApi, type DesktopApi, type RunMessage } from "../api/desktop";
import type { Value, WorkflowFile, WorkflowNode } from "../model/contracts";
import { createWorkflow, newId } from "../model/factory";
import { addNode, deleteNodes, setLayout, updateNode } from "../model/graph";
import {
  copyNodes,
  pasteNodes,
  type WorkflowClipboard,
} from "../model/clipboard";
import { INITIAL_STATE, type EditorTab, type StudioState } from "./state";
import { DocumentPersistence } from "./persistence";
import { WorkspaceSession } from "./workspace";
import { nodeUsage } from "../nodes/usage";
import { DocumentLibrary } from "./documents";
import { removeEdge } from "../model/connections";
import type { NodeConnection } from "../model/node-creation";

/** 编辑、文档历史和服务编排的唯一门面；组件不直接修改 Store。 */
export class WorkflowStudio {
  readonly store = createStore<StudioState>(() => INITIAL_STATE);
  private readonly persistence: DocumentPersistence;
  private readonly workspaceSession: WorkspaceSession;
  private readonly library: DocumentLibrary;
  private clipboard: WorkflowClipboard | null = null;
  constructor(readonly api: DesktopApi = desktopApi) {
    this.persistence = new DocumentPersistence(this.store, api);
    this.library = new DocumentLibrary(this.store, api, this.persistence);
    this.workspaceSession = new WorkspaceSession(this.store, api, (id) =>
      this.open(id),
    );
  }
  get active(): EditorTab | undefined {
    const state = this.store.getState();
    return state.active ? state.tabs[state.active] : undefined;
  }
  get readonly(): boolean {
    const state = this.store.getState();
    return (
      state.busy ||
      Boolean(
        state.run &&
        ["running", "cleaning"].includes(state.run.status) &&
        state.run.documents.includes(state.active ?? ""),
      )
    );
  }
  initializeWorkspace(): Promise<void> {
    return this.workspaceSession.initialize();
  }
  async open(id: string): Promise<void> {
    if (this.store.getState().tabs[id]) {
      this.store.setState({ active: id });
      return;
    }
    const loaded = await this.api.load(id);
    this.install(loaded.file, loaded.revision, false);
  }
  async refreshDocuments(): Promise<void> {
    this.store.setState({ documents: await this.api.listDocuments() });
  }
  renameDocument(id: string, name: string): Promise<void> {
    return this.library.rename(id, name);
  }
  deleteDocument(id: string): Promise<void> {
    return this.library.remove(id);
  }
  async reference(id: string): Promise<WorkflowFile> {
    const state = this.store.getState();
    const file = state.tabs[id]?.file ?? (await this.api.load(id)).file;
    this.store.setState({
      references: { ...this.store.getState().references, [id]: file },
    });
    return file;
  }
  private install(
    file: WorkflowFile,
    revision: string | null,
    dirty: boolean,
  ): void {
    const tab: EditorTab = {
      file,
      revision,
      version: dirty ? 1 : 0,
      savedVersion: 0,
      status: dirty ? "dirty" : "saved",
      past: [],
      future: [],
      scope: file.definition.root,
      selected: [],
      selectedEdge: null,
      viewport: { x: 260, y: 70, zoom: 1 },
    };
    this.store.setState({
      tabs: { ...this.store.getState().tabs, [file.id]: tab },
      active: file.id,
    });
    if (dirty) this.persistence.schedule(file.id);
  }
  create(file = createWorkflow()): void {
    if (this.store.getState().initialization.status !== "ready")
      throw new Error("工作流数据尚未就绪，请稍后重试");
    this.install(file, null, true);
  }
  async reload(): Promise<void> {
    const active = this.active;
    if (!active || this.readonly) return;
    const loaded = await this.api.load(active.file.id);
    this.install(loaded.file, loaded.revision, false);
  }
  saveCopy(): void {
    const active = this.active;
    if (active) {
      this.create({
        ...active.file,
        id: newId("flow"),
        definition: {
          ...active.file.definition,
          name: active.file.definition.name + " 副本",
        },
      });
      if (active.status === "conflict") {
        // 当前草稿已由新 ID 持有；移除冲突标签，后续保存不再被原文件阻塞。
        const tabs = { ...this.store.getState().tabs };
        delete tabs[active.file.id];
        this.store.setState({ tabs, message: null });
      }
    }
  }
  async close(id: string): Promise<void> {
    await this.persistence.flush(id);
    const state = this.store.getState();
    const tabs = { ...state.tabs };
    delete tabs[id];
    this.store.setState({
      tabs,
      active:
        state.active === id ? (Object.keys(tabs).at(-1) ?? null) : state.active,
    });
  }
  edit(update: (file: WorkflowFile) => WorkflowFile, history = true): void {
    const tab = this.active;
    if (!tab || this.readonly) return;
    const file = update(tab.file);
    if (file === tab.file) return;
    this.updateTab({
      ...tab,
      file,
      version: tab.version + 1,
      selectedEdge: file.definition.scopes.some(
        (scope) =>
          scope.id === tab.scope &&
          scope.edges.some((edge) => edge.id === tab.selectedEdge),
      )
        ? tab.selectedEdge
        : null,
      status: tab.status === "conflict" ? "conflict" : "dirty",
      past: history ? [...tab.past.slice(-99), tab.file] : tab.past,
      future: [],
    });
    this.persistence.schedule(file.id);
  }
  changeNode(id: string, update: (node: WorkflowNode) => WorkflowNode): void {
    this.edit((file) => updateNode(file, id, update));
  }
  rename(id: string, label: string): void {
    this.edit((file) => setLayout(file, id, { label }));
  }
  draft(
    id: string,
    field: string,
    value: string | null,
    update?: (file: WorkflowFile) => WorkflowFile,
  ): void {
    this.edit((file) => {
      const drafts = { ...file.editor.drafts };
      if (value === null) delete drafts[id + ":" + field];
      else drafts[id + ":" + field] = value;
      const next = update ? update(file) : file;
      return { ...next, editor: { ...next.editor, drafts } };
    });
  }
  add(
    kind: string,
    position: FlowPoint,
    after?: NodeConnection | null,
    scopeId?: string,
  ): void {
    const tab = this.active;
    if (!tab || this.readonly) return;
    let id = "";
    this.edit((file) => {
      const result = addNode(file, scopeId ?? tab.scope, kind, position, after);
      id = result.id;
      return result.file;
    });
    if (id) {
      this.select([id], scopeId ?? tab.scope);
      if (this.active?.file !== tab.file) nodeUsage.record(kind);
    }
  }
  remove(): void {
    const tab = this.active;
    if (tab) {
      this.edit((file) =>
        tab.selectedEdge
          ? removeEdge(file, tab.scope, tab.selectedEdge)
          : deleteNodes(file, new Set(tab.selected)),
      );
      this.select([]);
    }
  }
  undo(): void {
    const tab = this.active;
    const file = tab?.past.at(-1);
    if (!tab || !file || this.readonly) return;
    this.updateTab({
      ...tab,
      file,
      version: tab.version + 1,
      status: "dirty",
      past: tab.past.slice(0, -1),
      future: [tab.file, ...tab.future],
      selected: [],
      selectedEdge: null,
    });
    this.persistence.schedule(file.id);
  }
  redo(): void {
    const tab = this.active;
    const file = tab?.future[0];
    if (!tab || !file || this.readonly) return;
    this.updateTab({
      ...tab,
      file,
      version: tab.version + 1,
      status: "dirty",
      past: [...tab.past, tab.file],
      future: tab.future.slice(1),
      selected: [],
      selectedEdge: null,
    });
    this.persistence.schedule(file.id);
  }
  async copy(cut = false): Promise<void> {
    const tab = this.active;
    if (!tab) return;
    this.clipboard = copyNodes(tab.file, tab.scope, new Set(tab.selected));
    if (this.clipboard) {
      await this.api.copy(JSON.stringify(this.clipboard));
      if (cut && this.active?.file.id === tab.file.id) {
        this.edit((file) => deleteNodes(file, new Set(tab.selected)));
        this.select([]);
      }
    }
  }
  async paste(position: FlowPoint, scopeId?: string): Promise<void> {
    const tab = this.active;
    if (!tab || this.readonly) return;
    const targetScope = scopeId ?? tab.scope;
    const source = await this.api.paste();
    if (!source) return;
    const clipboard = await this.api.parseClipboard(source);
    if (
      this.active?.file.id !== tab.file.id ||
      !this.active.file.definition.scopes.some(
        (scope) => scope.id === targetScope,
      )
    )
      return;
    let selected: readonly string[] = [];
    this.edit((file) => {
      const result = pasteNodes(file, targetScope, clipboard, position);
      selected = result.selected;
      return result.file;
    });
    this.select(selected, targetScope);
  }
  duplicate(): void {
    const tab = this.active;
    if (!tab) return;
    const clipboard = copyNodes(tab.file, tab.scope, new Set(tab.selected));
    if (!clipboard) return;
    const first = tab.file.editor.nodes[tab.selected[0]];
    const result = pasteNodes(tab.file, tab.scope, clipboard, {
      x: first.x + 32,
      y: first.y + 32,
    });
    this.edit(() => result.file);
    this.select(result.selected);
  }
  /** 选择仅改变编辑归属，相机始终使用根场景坐标。 */
  select(selected: readonly string[], scope?: string): void {
    const tab = this.active;
    if (tab)
      this.updateTab({
        ...tab,
        selected,
        selectedEdge: null,
        scope: scope ?? tab.scope,
      });
  }
  /** 连线 ID 不混入节点选择，避免剪贴板与属性面板误读。 */
  selectEdge(id: string | null, scope: string): void {
    const tab = this.active;
    if (tab) this.updateTab({ ...tab, selected: [], selectedEdge: id, scope });
  }
  /** 更新根场景相机，不隐式切换编辑作用域或清空选择。 */
  view(viewport: ViewportTransform): void {
    if (
      !Number.isFinite(viewport.x) ||
      !Number.isFinite(viewport.y) ||
      !Number.isFinite(viewport.zoom) ||
      viewport.zoom <= 0
    )
      throw new Error("画布坐标或缩放无效");
    const tab = this.active;
    if (tab)
      this.updateTab({
        ...tab,
        viewport,
      });
  }
  panel(dock: StudioState["dock"], node?: string): void {
    this.store.setState({ dock, dockOpen: true, aqlNode: node ?? null });
  }
  toggleDock(): void {
    this.store.setState({ dockOpen: !this.store.getState().dockOpen });
  }
  message(message: string | null): void {
    this.store.setState({ message });
  }
  async safely(action: () => Promise<unknown> | unknown): Promise<void> {
    try {
      await action();
    } catch (error) {
      this.message(String(error));
    }
  }
  async flushAll(): Promise<void> {
    for (const id of Object.keys(this.store.getState().tabs))
      await this.persistence.flush(id);
  }
  async validate(): Promise<boolean> {
    await this.flushAll();
    const tab = this.active;
    if (!tab) return false;
    const problems = Object.entries(tab.file.editor.drafts).map(
      ([key, value]) => ({
        code: "draft",
        message: value,
        scope: null,
        node: key.split(":")[0],
        field: key.split(":").slice(1).join(":"),
      }),
    );
    const result = problems.length
      ? problems
      : await this.api.validate(tab.file.id);
    this.store.setState({
      problems: result,
      dock: "problems",
      dockOpen: result.length > 0 || this.store.getState().dockOpen,
    });
    if (!result.length) this.message("校验通过");
    return result.length === 0;
  }
  async run(inputs: Readonly<Record<string, Value>>): Promise<void> {
    if (!this.active || this.readonly) return;
    const id = this.active.file.id;
    this.store.setState({
      busy: true,
      dock: "logs",
      dockOpen: true,
      message: null,
    });
    try {
      await this.flushAll();
      const tab = this.store.getState().tabs[id];
      if (Object.keys(tab.file.editor.drafts).length)
        throw new Error("请先完成节点配置");
      const problems = await this.api.validate(id);
      if (problems.length) {
        this.store.setState({ problems, dock: "problems" });
        return;
      }
      const revisions: Record<string, string> = {};
      for (const item of Object.values(this.store.getState().tabs))
        if (item.revision) revisions[item.file.id] = item.revision;
      await this.api.start(id, inputs, revisions, this.receive);
    } finally {
      this.store.setState({ busy: false });
    }
  }
  async stop(): Promise<void> {
    await this.api.stop();
  }
  async subscribe(): Promise<void> {
    await this.api.subscribe(this.receive);
  }
  private readonly receive = (message: RunMessage): void => {
    const state = this.store.getState();
    if (message.type === "snapshot") {
      this.store.setState({
        run: message.snapshot,
        dockOpen: true,
        dock: message.snapshot.errors.length ? "logs" : state.dock,
      });
      return;
    }
    const run = state.run;
    if (!run || run.id !== message.id) return;
    if (message.type === "status")
      this.store.setState({ run: { ...run, status: message.status } });
    else if (
      !run.logs.some((entry) => entry.sequence === message.entry.sequence)
    ) {
      this.store.setState({
        run: {
          ...run,
          logs: [...run.logs.slice(-4999), message.entry],
          omitted: run.omitted + (run.logs.length >= 5000 ? 1 : 0),
        },
      });
    }
  };
  private updateTab(tab: EditorTab): void {
    if (!tab.file.definition.scopes.some((scope) => scope.id === tab.scope))
      tab = { ...tab, scope: tab.file.definition.root, selected: [] };
    this.store.setState({
      tabs: { ...this.store.getState().tabs, [tab.file.id]: tab },
    });
  }
}
export const studio = new WorkflowStudio();
