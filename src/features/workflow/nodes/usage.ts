import { createStore } from "zustand/vanilla";
import { NODE_CATALOG } from "./catalog";

/** 只有成功添加才记录；顺序号用于同频率时稳定按最近一次排序。 */
export interface NodeUsageEntry {
  /** 已成功添加的次数。 */
  readonly count: number;
  /** 最近添加的单调顺序号，不依赖系统时钟。 */
  readonly order: number;
}
/** 常用统计属于界面偏好，不写入工作流文档。 */
export interface NodeUsageState {
  /** 当前目录内的节点统计，以节点类型标识索引。 */
  readonly entries: Readonly<Record<string, NodeUsageEntry>>;
  /** 偏好读写失败提示，文档编辑仍可继续。 */
  readonly error: string | null;
}
/** 尚无统计时的推荐顺序，不伪造使用次数。 */
export const COMMON_NODE_DEFAULTS = [
  "start",
  "end",
  "let",
  "assign",
  "if",
  "for_each",
  "wait",
  "call_workflow",
] as const;
const STORAGE_KEY = "argusflow.nodeUsage";
type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

/** 解码偏好时只接受当前节点和有效计数，不兼容旧收藏结构。 */
function readEntries(
  storage?: PreferenceStorage,
): Readonly<Record<string, NodeUsageEntry>> {
  const source = storage?.getItem(STORAGE_KEY);
  if (!source) return {};
  const parsed: unknown = JSON.parse(source);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed))
    throw new Error("常用节点记录格式无效");
  const entries: Record<string, NodeUsageEntry> = {};
  for (const item of NODE_CATALOG) {
    if (!(item.id in parsed)) continue;
    const entry: unknown = Reflect.get(parsed, item.id);
    if (
      typeof entry !== "object" ||
      entry === null ||
      !("count" in entry) ||
      !("order" in entry) ||
      typeof entry.count !== "number" ||
      typeof entry.order !== "number" ||
      !Number.isSafeInteger(entry.count) ||
      entry.count <= 0 ||
      !Number.isSafeInteger(entry.order) ||
      entry.order <= 0
    )
      throw new Error("常用节点计数无效");
    entries[item.id] = { count: entry.count, order: entry.order };
  }
  return entries;
}

/** 独立响应式偏好服务；存储失败不撤销已经成功的节点编辑。 */
export class NodeUsage {
  readonly store = createStore<NodeUsageState>(() => ({
    entries: {},
    error: null,
  }));
  constructor(private readonly storage?: PreferenceStorage) {
    try {
      this.store.setState({ entries: readEntries(storage) });
    } catch {
      this.store.setState({
        error: "常用记录无法读取，将从本次使用重新统计。",
      });
    }
  }

  /** 成功添加后调用；撤销、重做、粘贴均不经过此入口。 */
  record(kind: string): void {
    if (!NODE_CATALOG.some((item) => item.id === kind)) return;
    const previous = this.store.getState().entries;
    const order = Math.min(
      Number.MAX_SAFE_INTEGER,
      Math.max(0, ...Object.values(previous).map((entry) => entry.order)) + 1,
    );
    const entries = {
      ...previous,
      [kind]: {
        count: Math.min(
          Number.MAX_SAFE_INTEGER,
          (previous[kind]?.count ?? 0) + 1,
        ),
        order,
      },
    };
    this.store.setState({ entries, error: null });
    try {
      this.storage?.setItem(STORAGE_KEY, JSON.stringify(entries));
    } catch {
      this.store.setState({ error: "常用记录未保存，本次使用仍会继续统计。" });
    }
  }
}

/** 排序结果只包含现有节点，未用过的推荐项补齐到八项。 */
export function commonNodes(entries: NodeUsageState["entries"]) {
  const ranked = NODE_CATALOG.filter((item) => entries[item.id]).sort(
    (a, b) =>
      entries[b.id].count - entries[a.id].count ||
      entries[b.id].order - entries[a.id].order,
  );
  const ids = [
    ...new Set([...ranked.map((item) => item.id), ...COMMON_NODE_DEFAULTS]),
  ].slice(0, 8);
  return ids.flatMap((id) => NODE_CATALOG.filter((item) => item.id === id));
}
/** 单窗口应用共享一份偏好；不触及旧收藏键。 */
export const nodeUsage = new NodeUsage(
  typeof localStorage === "undefined" ? undefined : localStorage,
);
