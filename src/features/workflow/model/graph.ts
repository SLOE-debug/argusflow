import type {
  Action,
  NodeLayout,
  Scope,
  WorkflowFile,
  WorkflowNode,
} from "./contracts";
import { childScopes, createNode } from "./factory";
import { NODE_CATALOG } from "../nodes/catalog";
import { defaultValue, literal } from "./expressions";

export function scopeById(file: WorkflowFile, id: string): Scope {
  const scope = file.definition.scopes.find((item) => item.id === id);
  if (!scope) throw new Error("作用域不存在");
  return scope;
}
export function nodeById(
  file: WorkflowFile,
  id: string,
): WorkflowNode | undefined {
  return file.definition.scopes
    .flatMap((scope) => scope.nodes)
    .find((node) => node.id === id);
}
export function nodeKind(node: WorkflowNode): string {
  return node.action.kind === "task"
    ? node.action.task.type_id
    : node.action.kind;
}
export function nodeTitle(file: WorkflowFile, node: WorkflowNode): string {
  return (
    file.editor.nodes[node.id]?.label ||
    NODE_CATALOG.find((item) => item.id === nodeKind(node))?.title ||
    node.action.kind
  );
}
export function replaceScope(file: WorkflowFile, scope: Scope): WorkflowFile {
  return {
    ...file,
    definition: {
      ...file.definition,
      scopes: file.definition.scopes.map((item) =>
        item.id === scope.id ? scope : item,
      ),
    },
  };
}
export function updateNode(
  file: WorkflowFile,
  id: string,
  update: (node: WorkflowNode) => WorkflowNode,
): WorkflowFile {
  return {
    ...file,
    definition: {
      ...file.definition,
      scopes: file.definition.scopes.map((scope) => ({
        ...scope,
        nodes: scope.nodes.map((node) =>
          node.id === id ? update(node) : node,
        ),
      })),
    },
  };
}
export function setLayout(
  file: WorkflowFile,
  id: string,
  update: Partial<NodeLayout>,
): WorkflowFile {
  return {
    ...file,
    editor: {
      ...file.editor,
      nodes: {
        ...file.editor.nodes,
        [id]: {
          ...(file.editor.nodes[id] ?? { x: 80, y: 100, label: "", note: "" }),
          ...update,
        },
      },
    },
  };
}
/** 添加、插线、连接采用同一图事务；新节点默认接到当前顺序末尾。 */
export function addNode(
  file: WorkflowFile,
  scopeId: string,
  kind: string,
  position: { readonly x: number; readonly y: number },
  after?: string | null,
): { readonly file: WorkflowFile; readonly id: string } {
  const scope = scopeById(file, scopeId);
  const created = createNode(kind);
  const previous =
    after === undefined
      ? scope.nodes.find(
          (node) =>
            node.next === null &&
            !["return", "fail", "break", "continue"].includes(node.action.kind),
        )?.id
      : after;
  const oldNext =
    previous === "$entry"
      ? scope.entry
      : previous
        ? (scope.nodes.find((node) => node.id === previous)?.next ?? null)
        : null;
  const terminal = ["return", "fail", "break", "continue"].includes(
    created.node.action.kind,
  );
  if (terminal && oldNext) throw new Error("终止节点只能放在步骤末尾");
  const node = {
    ...created.node,
    next: oldNext,
    action:
      created.node.action.kind === "return"
        ? {
            ...created.node.action,
            values: Object.fromEntries(
              Object.entries(file.definition.outputs).map(([name, type]) => [
                name,
                literal(type, defaultValue(type)),
              ]),
            ),
          }
        : created.node.action,
  };
  const nodes = [
    ...scope.nodes.map((item) =>
      item.id === previous ? { ...item, next: node.id } : item,
    ),
    node,
  ];
  let next = replaceScope(file, {
    ...scope,
    entry: previous === "$entry" ? node.id : (scope.entry ?? node.id),
    nodes,
  });
  next = {
    ...next,
    definition: {
      ...next.definition,
      scopes: [...next.definition.scopes, ...created.scopes],
    },
  };
  return {
    file: setLayout(next, node.id, {
      ...position,
      label: NODE_CATALOG.find((item) => item.id === kind)?.title ?? kind,
    }),
    id: node.id,
  };
}
export function connectNodes(
  file: WorkflowFile,
  scopeId: string,
  from: string,
  to: string | null,
): WorkflowFile {
  const scope = scopeById(file, scopeId);
  if (from === "$entry") {
    if (to !== null && !scope.nodes.some((node) => node.id === to))
      throw new Error("入口必须连接当前作用域");
    return replaceScope(file, { ...scope, entry: to });
  }
  const source = scope.nodes.find((node) => node.id === from);
  if (!source || (to && !scope.nodes.some((node) => node.id === to)))
    throw new Error("不能跨作用域连线");
  if (
    to &&
    ["return", "fail", "break", "continue"].includes(source.action.kind)
  )
    throw new Error("终止节点不能继续连线");
  if (to && scope.nodes.some((node) => node.id !== from && node.next === to))
    throw new Error("目标已有前置步骤；可在线路上插入节点");
  let cursor = to;
  const visited = new Set<string>();
  while (cursor) {
    if (cursor === from || visited.has(cursor))
      throw new Error("不能创建回环，请使用循环节点");
    visited.add(cursor);
    cursor = scope.nodes.find((node) => node.id === cursor)?.next ?? null;
  }
  return updateNode(file, from, (node) => ({ ...node, next: to }));
}
/** 删除容器时一并移除其拥有的子图，顺序链重新接回最近保留节点。 */
export function deleteNodes(
  file: WorkflowFile,
  selected: ReadonlySet<string>,
): WorkflowFile {
  const removed = new Set(selected);
  const scopes = new Set<string>();
  const visit = (action: Action) =>
    childScopes(action).forEach(({ id }) => {
      scopes.add(id);
      scopeById(file, id).nodes.forEach((node) => {
        removed.add(node.id);
        visit(node.action);
      });
    });
  file.definition.scopes.forEach((scope) =>
    scope.nodes
      .filter((node) => selected.has(node.id))
      .forEach((node) => visit(node.action)),
  );
  const layout = Object.fromEntries(
    Object.entries(file.editor.nodes).filter(([id]) => !removed.has(id)),
  );
  const drafts = Object.fromEntries(
    Object.entries(file.editor.drafts).filter(
      ([key]) => !removed.has(key.split(":")[0]),
    ),
  );
  return {
    ...file,
    editor: { nodes: layout, drafts },
    definition: {
      ...file.definition,
      scopes: file.definition.scopes
        .filter((scope) => !scopes.has(scope.id))
        .map((scope) => {
          const skip = (id: string | null): string | null => {
            const seen = new Set<string>();
            while (id && removed.has(id)) {
              if (seen.has(id)) return null;
              seen.add(id);
              id = scope.nodes.find((node) => node.id === id)?.next ?? null;
            }
            return id;
          };
          return {
            ...scope,
            entry: skip(scope.entry),
            nodes: scope.nodes
              .filter((node) => !removed.has(node.id))
              .map((node) => ({ ...node, next: skip(node.next) })),
          };
        }),
    },
  };
}
