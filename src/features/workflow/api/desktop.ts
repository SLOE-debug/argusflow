import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type {
  DocumentSummary,
  JsonValue,
  Problem,
  Value,
  ValueType,
  WorkflowFile,
} from "../model/contracts";
import type { WorkflowClipboard } from "../model/clipboard";

export interface LoadedDocument {
  readonly file: WorkflowFile;
  readonly revision: string;
}
export interface RunLocation {
  readonly workflow: string | null;
  readonly scope: string;
  readonly node: string | null;
  readonly instance: string;
  readonly execution: string | null;
}
export interface LogEntry {
  readonly sequence: string;
  readonly elapsed_ms: string;
  readonly kind: string;
  readonly level: string;
  readonly message: string;
  readonly path: readonly RunLocation[];
}
export type RunStatus =
  "running" | "cleaning" | "completed" | "failed" | "cancelled" | "timed_out";
export interface RunSnapshot {
  readonly id: string;
  readonly workflow: string;
  readonly documents: readonly string[];
  readonly status: RunStatus;
  readonly logs: readonly LogEntry[];
  readonly omitted: number;
  readonly outputs: Readonly<Record<string, Value>>;
  readonly errors: readonly string[];
}
export type RunMessage =
  | { readonly type: "snapshot"; readonly snapshot: RunSnapshot }
  | { readonly type: "log"; readonly id: string; readonly entry: LogEntry }
  | {
      readonly type: "status";
      readonly id: string;
      readonly status: RunStatus;
    };
/** 可替换的传输边界，组件测试不需要启动 WebView。 */
export interface DesktopApi {
  initializeWorkspace(): Promise<{
    readonly path: string;
    readonly documents: readonly DocumentSummary[];
  }>;
  listDocuments(): Promise<readonly DocumentSummary[]>;
  load(id: string): Promise<LoadedDocument>;
  save(file: WorkflowFile, revision: string | null): Promise<LoadedDocument>;
  validate(id: string): Promise<readonly Problem[]>;
  start(
    id: string,
    inputs: Readonly<Record<string, Value>>,
    revisions: Readonly<Record<string, string>>,
    receive: (message: RunMessage) => void,
  ): Promise<string>;
  stop(): Promise<void>;
  capabilities(): Promise<readonly string[]>;
  subscribe(receive: (message: RunMessage) => void): Promise<void>;
  copy(text: string): Promise<void>;
  paste(): Promise<string | null>;
  parseClipboard(source: string): Promise<WorkflowClipboard>;
  exportLog(text: string): Promise<void>;
  describeTask(
    typeId: string,
    config: Readonly<Record<string, JsonValue>>,
  ): Promise<Readonly<Record<string, ValueType>>>;
}
function ensureDesktop(): void {
  if (!isTauri()) throw new Error("请使用 pnpm tauri dev 启动桌面工作台");
}
function channel(receive: (message: RunMessage) => void): Channel<RunMessage> {
  return new Channel<RunMessage>(receive);
}
export const desktopApi: DesktopApi = {
  describeTask: (typeId, config) => invoke("describe_task", { typeId, config }),
  async initializeWorkspace() {
    ensureDesktop();
    return invoke("initialize_workspace");
  },
  listDocuments: () => invoke("list_documents"),
  load: (id) => invoke("load_document", { id }),
  save: (file, revision) => invoke("save_document", { file, revision }),
  validate: (id) => invoke("validate_workflow", { id }),
  start: (id, inputs, revisions, receive) =>
    invoke("start_workflow", {
      id,
      inputs,
      revisions,
      channel: channel(receive),
    }),
  capabilities: () => invoke("capabilities"),
  stop: () => invoke("stop_workflow"),
  subscribe: (receive) => invoke("get_run", { channel: channel(receive) }),
  copy: (text) => writeText(text),
  paste: () => readText(),
  parseClipboard: (source) => invoke("parse_node_clipboard", { source }),
  async exportLog(text) {
    const path = await save({
      title: "导出运行日志",
      defaultPath: "workflow.log",
      filters: [{ name: "日志文件", extensions: ["log"] }],
    });
    if (path) await invoke("export_log", { path, text });
  },
};
