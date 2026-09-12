import type {
  NodeLayout,
  Scope,
  WorkflowFile,
  WorkflowNode,
} from "./contracts";
import { NODE_CATALOG } from "../nodes/catalog";

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
export { addNode } from "./node-creation";
export { deleteNodes } from "./node-deletion";
