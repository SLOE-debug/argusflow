import type { ViewportTransform } from "../../../flow";
import type {
  DocumentSummary,
  Problem,
  WorkflowFile,
} from "../model/contracts";
import type { RunSnapshot } from "../api/desktop";
export type SaveStatus = "saved" | "dirty" | "saving" | "failed" | "conflict";
export interface EditorTab {
  readonly file: WorkflowFile;
  readonly revision: string | null;
  readonly version: number;
  readonly savedVersion: number;
  readonly status: SaveStatus;
  readonly error?: string;
  readonly past: readonly WorkflowFile[];
  readonly future: readonly WorkflowFile[];
  /** 编辑归属，不控制场景显示层级。 */
  readonly scope: string;
  readonly selected: readonly string[];
  /** 连线选择与节点集合互斥，身份属于当前编辑作用域。 */
  readonly selectedEdge: string | null;
  /** 从根场景到画布 CSS 像素的相机变换。 */
  readonly viewport: ViewportTransform;
}
export type DockTab = "logs" | "problems" | "data" | "aql";
/** 应用数据初始化失败可以重试，错误不会伪装为空工作区。 */
export type WorkspaceInitialization =
  | { readonly status: "idle" | "loading" | "ready" }
  | { readonly status: "failed"; readonly error: string };
export interface StudioState {
  readonly initialization: WorkspaceInitialization;
  readonly workspace: string | null;
  readonly documents: readonly DocumentSummary[];
  readonly tabs: Readonly<Record<string, EditorTab>>;
  readonly references: Readonly<Record<string, WorkflowFile>>;
  readonly active: string | null;
  readonly run: RunSnapshot | null;
  readonly problems: readonly Problem[];
  readonly message: string | null;
  readonly busy: boolean;
  readonly dock: DockTab;
  readonly dockOpen: boolean;
  readonly aqlNode: string | null;
}
export const INITIAL_STATE: StudioState = {
  initialization: { status: "idle" },
  workspace: null,
  documents: [],
  tabs: {},
  references: {},
  active: null,
  run: null,
  problems: [],
  message: null,
  busy: false,
  dock: "logs",
  dockOpen: false,
  aqlNode: null,
};
