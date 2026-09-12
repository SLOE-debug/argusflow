import type {
  EdgeEndpoint,
  EdgeLayout,
  Scope,
  WorkflowEdge,
  WorkflowFile,
} from "./contracts";
import { edgeEndpoint, endpointNodeId } from "./endpoints";
import { newId } from "./factory";
import { replaceScope, scopeById } from "./graph";

/** 同一节点的四个端口不改变连接身份。 */
export function endpointKey(endpoint: EdgeEndpoint): string {
  return endpoint.kind === "node" ? "node:" + endpoint.node : endpoint.kind;
}
/** 终止动作只接收入线。容器是否正常完成由运行编译判断。 */
export function isTerminal(kind: string): boolean {
  return ["return", "fail", "break", "continue"].includes(kind);
}

/** 邻接查询集中管理，非线性草稿从不隐式选择第一条出口。 */
export class GraphIndex {
  private readonly inputs = new Map<string, WorkflowEdge[]>();
  private readonly outputs = new Map<string, WorkflowEdge[]>();
  constructor(readonly scope: Scope) {
    for (const edge of scope.edges) {
      const source = endpointKey(edge.source),
        target = endpointKey(edge.target);
      const outputs = this.outputs.get(source),
        inputs = this.inputs.get(target);
      if (outputs) outputs.push(edge);
      else this.outputs.set(source, [edge]);
      if (inputs) inputs.push(edge);
      else this.inputs.set(target, [edge]);
    }
  }
  outgoing(endpoint: EdgeEndpoint): readonly WorkflowEdge[] {
    return this.outputs.get(endpointKey(endpoint)) ?? [];
  }
  incoming(endpoint: EdgeEndpoint): readonly WorkflowEdge[] {
    return this.inputs.get(endpointKey(endpoint)) ?? [];
  }
  successor(endpoint: EdgeEndpoint): EdgeEndpoint | undefined {
    const edges = this.outgoing(endpoint);
    return edges.length === 1 ? edges[0].target : undefined;
  }
  /** 从开始到首次分叉的确定顺序，供草稿符号候选和线性操作使用。 */
  prefix(): readonly string[] {
    const result: string[] = [],
      seen = new Set<string>();
    let next = this.successor({ kind: "start" });
    while (next?.kind === "node" && !seen.has(next.node)) {
      seen.add(next.node);
      result.push(next.node);
      next = this.successor(next);
    }
    return result;
  }
}
const indexes = new WeakMap<Scope, GraphIndex>();
/** 不可变作用域快照可以跨选择、缩放及指针预览复用。 */
export function graphIndex(scope: Scope): GraphIndex {
  let index = indexes.get(scope);
  if (!index) {
    index = new GraphIndex(scope);
    indexes.set(scope, index);
  }
  return index;
}

/** 预览和提交使用相同规则；返回可直接展示的无效落点原因。 */
export function connectionError(
  file: WorkflowFile,
  scopeId: string,
  source: EdgeEndpoint,
  target: EdgeEndpoint,
  excluding?: string,
): string | null {
  const scope = scopeById(file, scopeId);
  if (source.kind === "end" || target.kind === "start")
    return "开始只能出线，结束只能接收入线";
  for (const endpoint of [source, target]) {
    if (
      endpoint.kind === "node"
        ? !scope.nodes.some((node) => node.id === endpoint.node)
        : !file.editor.nodes[endpointNodeId(scopeId, endpoint)]
    )
      return "端点必须属于当前作用域";
  }
  if (
    source.kind === "node" &&
    isTerminal(scope.nodes.find((node) => node.id === source.node)!.action.kind)
  )
    return "终止节点不能继续连线";
  const from = endpointKey(source),
    to = endpointKey(target);
  if (from === to) return "不能连接节点自身";
  const edges = scope.edges.filter((edge) => edge.id !== excluding);
  if (
    edges.some(
      (edge) =>
        endpointKey(edge.source) === from && endpointKey(edge.target) === to,
    )
  )
    return "这两个节点已连接";
  const index = new GraphIndex({ ...scope, edges });
  const pending = [target],
    seen = new Set<string>();
  while (pending.length) {
    const item = pending.pop()!,
      key = endpointKey(item);
    if (key === from) return "不能创建回环，请使用循环节点";
    if (seen.has(key)) continue;
    seen.add(key);
    pending.push(...index.outgoing(item).map((edge) => edge.target));
  }
  return null;
}

/** 新连接只追加，绝不覆盖已有出口。 */
export function createConnection(
  file: WorkflowFile,
  scopeId: string,
  from: string,
  to: string,
  layout: EdgeLayout = { source: "right", target: "left" },
): WorkflowFile {
  const source = edgeEndpoint(scopeId, from),
    target = edgeEndpoint(scopeId, to);
  const reason = connectionError(file, scopeId, source, target);
  if (reason) throw new Error(reason);
  const edge: WorkflowEdge = { id: newId("edge"), source, target };
  const scope = scopeById(file, scopeId);
  const next = replaceScope(file, { ...scope, edges: [...scope.edges, edge] });
  return {
    ...next,
    editor: {
      ...next.editor,
      edges: { ...next.editor.edges, [edge.id]: layout },
    },
  };
}

/** 原子改接一个端点，同一节点换边也保留边身份。 */
export function reconnectEdge(
  file: WorkflowFile,
  scopeId: string,
  id: string,
  end: "source" | "target",
  node: string,
  side: EdgeLayout["source"],
): WorkflowFile {
  const scope = scopeById(file, scopeId),
    edge = scope.edges.find((edge) => edge.id === id);
  if (!edge) throw new Error("连线已不存在");
  const changed = { ...edge, [end]: edgeEndpoint(scopeId, node) };
  const reason = connectionError(
    file,
    scopeId,
    changed.source,
    changed.target,
    id,
  );
  if (reason) throw new Error(reason);
  const layout = file.editor.edges[id];
  if (
    endpointKey(edge[end]) === endpointKey(changed[end]) &&
    layout[end] === side
  )
    return file;
  const next = replaceScope(file, {
    ...scope,
    edges: scope.edges.map((item) => (item.id === id ? changed : item)),
  });
  return {
    ...next,
    editor: {
      ...next.editor,
      edges: { ...next.editor.edges, [id]: { ...layout, [end]: side } },
    },
  };
}

/** 删除指定边及其布局，不修改其他前后继。 */
export function removeEdge(
  file: WorkflowFile,
  scopeId: string,
  id: string,
): WorkflowFile {
  const scope = scopeById(file, scopeId);
  if (!scope.edges.some((edge) => edge.id === id)) return file;
  const next = replaceScope(file, {
    ...scope,
    edges: scope.edges.filter((edge) => edge.id !== id),
  });
  const layouts = { ...file.editor.edges };
  delete layouts[id];
  return { ...next, editor: { ...next.editor, edges: layouts } };
}
