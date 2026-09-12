import type { EdgeLayout, WorkflowFile } from "./contracts";
import { createNode, newId } from "./factory";
import { scopeById, replaceScope, setLayout } from "./graph";
import { defaultValue, literal } from "./expressions";
import {
  endpointId,
  initialScopeLayout,
  isEndpointKind,
  scopeEndpoints,
} from "./endpoints";
import { createConnection, graphIndex, isTerminal } from "./connections";
import { buildScene } from "./layout";
import { NODE_CATALOG } from "../nodes/catalog";
import { facingSide, rectAnchor, type FlowPoint } from "../../../flow";

/** 搜索既可拆分指定边，也可从指定端口追加一条独立连接。 */
export type NodeConnection =
  | { readonly kind: "insert"; readonly edge: string }
  | {
      readonly kind: "from";
      readonly node: string;
      readonly side: EdgeLayout["source"];
    };

/** 常规添加只在确定的线性结束线上插入；分支图不猜测出口。 */
export function addNode(
  file: WorkflowFile,
  scopeId: string,
  kind: string,
  position: FlowPoint,
  connection?: NodeConnection | null,
): { readonly file: WorkflowFile; readonly id: string } {
  const scope = scopeById(file, scopeId);
  if (isEndpointKind(kind)) {
    if (connection) throw new Error("起止不能作为线路中的步骤插入");
    const id = endpointId(scopeId, kind);
    return {
      id,
      file: file.editor.nodes[id]
        ? file
        : setLayout(file, id, {
            ...position,
            label: kind === "start" ? "开始" : "结束",
          }),
    };
  }
  const created = createNode(kind);
  const terminal = isTerminal(created.node.action.kind);
  const index = graphIndex(scope);
  const ending = scope.edges.filter((edge) => edge.target.kind === "end");
  const linear =
    index.prefix().length === scope.nodes.length &&
    scope.edges.every((edge) => index.outgoing(edge.source).length === 1);
  const split =
    connection?.kind === "insert"
      ? scope.edges.find((edge) => edge.id === connection.edge)
      : connection === undefined && linear && ending.length === 1
        ? ending[0]
        : undefined;
  if (connection?.kind === "insert" && !split) throw new Error("连线已不存在");
  if (terminal && split?.target.kind === "node")
    throw new Error("终止节点只能放在步骤末尾");
  const node =
    created.node.action.kind === "return"
      ? {
          ...created.node,
          action: {
            ...created.node.action,
            values: Object.fromEntries(
              Object.entries(file.definition.outputs).map(([name, type]) => [
                name,
                literal(type, defaultValue(type)),
              ]),
            ),
          },
        }
      : created.node;
  let next = replaceScope(file, { ...scope, nodes: [...scope.nodes, node] });
  const layouts = { ...file.editor.nodes },
    edgeLayouts = { ...file.editor.edges };
  for (const child of created.scopes) {
    Object.assign(layouts, initialScopeLayout(child.id));
    for (const edge of child.edges)
      edgeLayouts[edge.id] = { source: "right", target: "left" };
  }
  next = {
    ...next,
    definition: {
      ...next.definition,
      scopes: [...next.definition.scopes, ...created.scopes],
    },
    editor: { ...next.editor, nodes: layouts, edges: edgeLayouts },
  };
  next = setLayout(next, node.id, {
    ...position,
    label: NODE_CATALOG.find((item) => item.id === kind)?.title ?? kind,
  });
  if (split) {
    const original = next.editor.edges[split.id];
    const edges = scopeById(next, scopeId).edges.map((edge) =>
      edge.id === split.id
        ? { ...edge, target: { kind: "node" as const, node: node.id } }
        : edge,
    );
    const tail = {
      id: newId("edge"),
      source: { kind: "node" as const, node: node.id },
      target: split.target,
    };
    if (!terminal) edges.push(tail);
    next = replaceScope(next, { ...scopeById(next, scopeId), edges });
    next = {
      ...next,
      editor: {
        ...next.editor,
        edges: {
          ...next.editor.edges,
          [split.id]: { source: original.source, target: "left" },
          ...(!terminal
            ? {
                [tail.id]: {
                  source: "right" as const,
                  target: original.target,
                },
              }
            : {}),
        },
      },
    };
    // 自动追加时将结束标记移到新卡片之外，避免默认节点堆叠。
    const end = endpointId(scopeId, "end");
    if (
      connection === undefined &&
      next.editor.nodes[end] &&
      split.target.kind === "end"
    ) {
      const placed = buildScene(next).scopes[scopeId].nodes.find(
        (item) => item.id === node.id,
      )!;
      next = setLayout(next, end, {
        x: Math.max(next.editor.nodes[end].x, placed.x + placed.width + 80),
        y: position.y + 4,
      });
    }
  } else if (connection?.kind === "from") {
    const geometry = buildScene(next).scopes[scopeId];
    const source = [...geometry.nodes, ...scopeEndpoints(next, scopeId)].find(
      (rect) => rect.id === connection.node,
    )!;
    const target = geometry.nodes.find((rect) => rect.id === node.id)!;
    const side = facingSide(
      { x: target.x + target.width / 2, y: target.y + target.height / 2 },
      rectAnchor(source, connection.side).point,
    );
    next = createConnection(next, scopeId, connection.node, node.id, {
      source: connection.side,
      target: side,
    });
  }
  return { file: next, id: node.id };
}
