import type {
  Expr,
  WorkflowFile,
  WorkflowNode,
  EdgeEndpoint,
} from "./contracts";
import { childScopes, newId } from "./factory";
import { scopeById, replaceScope } from "./graph";
import { availableSymbols } from "../values/symbols";
import { rewriteAction, rewriteExpression } from "./clipboard-rewrite";
import { bindingDeclarations } from "./bindings";
import { endpointId, scopeEndpoints, endpointNodeId } from "./endpoints";
import { endpointKey } from "./connections";

export interface WorkflowClipboard {
  readonly format: "argusflow.nodes";
  readonly sourceWorkflow: string;
  readonly sourceScope: string;
  readonly selected: readonly string[];
  readonly bindings: Readonly<Record<string, Readonly<Record<string, string>>>>;
  readonly file: WorkflowFile;
}
/** 仅复制选中节点和它们拥有的内部图。 */
export function copyNodes(
  file: WorkflowFile,
  scopeId: string,
  selected: ReadonlySet<string>,
): WorkflowClipboard | null {
  const scope = scopeById(file, scopeId);
  const roots = scope.nodes.filter((node) => selected.has(node.id));
  const endpoints = scopeEndpoints(file, scopeId).filter((node) =>
    selected.has(node.id),
  );
  if (!roots.length && !endpoints.length) return null;
  const children = new Set<string>();
  const visit = (node: WorkflowNode) =>
    childScopes(node.action).forEach(({ id }) => {
      if (children.has(id)) throw new Error("结构容器重复引用子图");
      children.add(id);
      scopeById(file, id).nodes.forEach(visit);
    });
  roots.forEach(visit);
  const scopes = [
    {
      ...scope,
      edges: scope.edges.filter(
        (edge) =>
          selected.has(endpointNodeId(scopeId, edge.source)) &&
          selected.has(endpointNodeId(scopeId, edge.target)),
      ),
      outputs: {},
      nodes: roots,
    },
    ...file.definition.scopes.filter((item) => children.has(item.id)),
  ];
  const ids = new Set(
    scopes.flatMap((item) => item.nodes.map((node) => node.id)),
  );
  endpoints.forEach((item) => ids.add(item.id));
  children.forEach((scope) =>
    scopeEndpoints(file, scope).forEach((item) => ids.add(item.id)),
  );
  return {
    format: "argusflow.nodes",
    sourceWorkflow: file.id,
    sourceScope: scopeId,
    selected: [...roots, ...endpoints].map((node) => node.id),
    bindings: Object.fromEntries(
      scopes.map((scope) => [scope.id, bindingDeclarations(file, scope.id)]),
    ),
    file: {
      ...file,
      definition: { ...file.definition, root: scopeId, scopes, subflows: {} },
      editor: {
        edges: Object.fromEntries(
          scopes.flatMap((scope) =>
            scope.edges.map((edge) => [edge.id, file.editor.edges[edge.id]]),
          ),
        ),
        nodes: Object.fromEntries(
          Object.entries(file.editor.nodes).filter(([id]) => ids.has(id)),
        ),
        drafts: Object.fromEntries(
          Object.entries(file.editor.drafts).filter(([key]) =>
            ids.has(key.split(":")[0]),
          ),
        ),
      },
    },
  };
}
/** 新作用域使用独立身份；变量和资源声明同步改名，调用工作流的身份保持不变。 */
export function pasteNodes(
  target: WorkflowFile,
  scopeId: string,
  clipboard: WorkflowClipboard,
  at: { readonly x: number; readonly y: number },
): { readonly file: WorkflowFile; readonly selected: readonly string[] } {
  const source = clipboard.file;
  const edgeIds = new Map(
    source.definition.scopes.flatMap((scope) =>
      scope.edges.map((edge) => [edge.id, newId("edge")] as const),
    ),
  );
  const ids = new Map(
    source.definition.scopes.flatMap((scope) =>
      scope.nodes.map((node) => [node.id, newId("node")] as const),
    ),
  );
  const scopes = new Map(
    source.definition.scopes.map((scope) => [
      scope.id,
      scope.id === clipboard.sourceScope ? scopeId : newId("scope"),
    ]),
  );
  for (const scope of source.definition.scopes)
    for (const endpoint of scopeEndpoints(source, scope.id)) {
      const id = endpointId(scopes.get(scope.id)!, endpoint.kind);
      // 粘贴到已有起止标记的作用域时保留原标记及其位置。
      if (!target.editor.nodes[id]) ids.set(endpoint.id, id);
    }
  if (!ids.size) {
    const existing = scopeById(target, scopeId).edges;
    const changed = scopeById(source, clipboard.sourceScope).edges.some(
      (edge) =>
        !existing.some(
          (item) =>
            endpointKey(item.source) === endpointKey(edge.source) &&
            endpointKey(item.target) === endpointKey(edge.target),
        ),
    );
    if (!changed) return { file: target, selected: [] };
  }
  const crossScope =
    clipboard.sourceWorkflow !== target.id || clipboard.sourceScope !== scopeId;
  const drafts: Record<string, string> = { ...target.editor.drafts };
  const parents = new Map<string, string>();
  const variables = new Map<string, Map<string, string>>();
  const resources = new Map<string, Map<string, string>>();
  const visible = availableSymbols(target, scopeId);
  const targetBindings = bindingDeclarations(target, scopeId);
  const keepsBinding = (
    scope: string,
    kind: "variable" | "resource",
    name: string,
  ) =>
    clipboard.sourceWorkflow === target.id &&
    clipboard.bindings[scope]?.[kind + "/" + name] !== undefined &&
    clipboard.bindings[scope][kind + "/" + name] ===
      targetBindings[kind + "/" + name] &&
    (kind === "resource"
      ? visible.resources.some((item) => item.name === name)
      : visible.values.some(
          (item) =>
            item.expression.kind === "variable" &&
            item.expression.name === name,
        ));
  const used = new Set([
    ...visible.values
      .filter((item) => item.expression.kind === "variable")
      .map((item) =>
        item.expression.kind === "variable" ? item.expression.name : "",
      ),
    ...scopeById(target, scopeId).nodes.flatMap((node) =>
      node.action.kind === "let" ? [node.action.name] : [],
    ),
    ...visible.resources.map((item) => item.name),
  ]);
  const unique = (name: string, root: boolean) => {
    if (!root) return name;
    let next = name,
      suffix = 2;
    while (used.has(next)) next = name + "_" + suffix++;
    used.add(next);
    return next;
  };
  for (const scope of source.definition.scopes) {
    const vars = variables.get(scope.id) ?? new Map<string, string>();
    const res = new Map<string, string>();
    for (const node of scope.nodes) {
      const action = node.action;
      if (action.kind === "let")
        vars.set(
          action.name,
          unique(action.name, scope.id === clipboard.sourceScope),
        );
      if (action.kind === "task")
        Object.values(action.task.resource_outputs).forEach((name) =>
          res.set(name, unique(name, scope.id === clipboard.sourceScope)),
        );
      childScopes(action).forEach((child) => parents.set(child.id, scope.id));
      if (action.kind === "for_each")
        variables.set(
          action.body,
          new Map([
            [action.item, action.item],
            [action.index, action.index],
          ]),
        );
    }
    variables.set(scope.id, vars);
    resources.set(scope.id, res);
  }
  const lookup = (
    map: ReadonlyMap<string, ReadonlyMap<string, string>>,
    scope: string,
    name: string,
  ): string | undefined => {
    const own = map.get(scope)?.get(name);
    return (
      own ??
      (parents.has(scope) ? lookup(map, parents.get(scope)!, name) : undefined)
    );
  };
  const copied = source.definition.scopes.map((scope) => {
    const rewriteFor = (nodeId: string) => {
      const mark = (field: string, name: string) => {
        drafts[nodeId + ":" + field] = "请重新绑定引用：" + name;
      };
      const variable = (name: string, field: string) => {
        const own = lookup(variables, scope.id, name);
        if (!own && crossScope && !keepsBinding(scope.id, "variable", name))
          mark(field, name);
        return own ?? name;
      };
      return {
        scope: (id: string) => scopes.get(id)!,
        variable,
        resource: (name: string, field: string) => {
          const own = lookup(resources, scope.id, name);
          if (!own && crossScope && !keepsBinding(scope.id, "resource", name))
            mark(field, name);
          return own ?? name;
        },
        expression: (expr: Expr, field: string) =>
          rewriteExpression(
            expr,
            ids,
            (name) => variable(name, field),
            (external) => {
              if (external.kind === "node_output") {
                if (
                  !visible.values.some(
                    (item) =>
                      item.expression.kind === "node_output" &&
                      item.expression.node === external.node &&
                      item.expression.output === external.output,
                  )
                )
                  mark(field, external.node);
              } else if (
                external.kind === "input" &&
                clipboard.sourceWorkflow !== target.id
              )
                mark(field, external.name);
            },
          ),
      };
    };
    return {
      ...scope,
      id: scopes.get(scope.id)!,
      edges: scope.edges.map((edge) => {
        const remap = (endpoint: EdgeEndpoint): EdgeEndpoint =>
          endpoint.kind === "node"
            ? { kind: "node", node: ids.get(endpoint.node)! }
            : endpoint;
        return {
          id: edgeIds.get(edge.id)!,
          source: remap(edge.source),
          target: remap(edge.target),
        };
      }),
      nodes: scope.nodes.map((node) => {
        const id = ids.get(node.id)!,
          rewrite = rewriteFor(id);
        return {
          ...node,
          id,
          action: rewriteAction(node.action, rewrite),
          output_bindings: Object.fromEntries(
            Object.entries(node.output_bindings).map(([name, expr]) => [
              name,
              rewrite.expression(expr, "output." + name),
            ]),
          ),
        };
      }),
      outputs: Object.fromEntries(
        Object.entries(scope.outputs).map(([name, expr]) => [
          name,
          rewriteFor(
            ids.get(
              source.definition.scopes
                .flatMap((s) => s.nodes)
                .find((node) =>
                  childScopes(node.action).some(
                    (child) => child.id === scope.id,
                  ),
                )?.id ?? "",
            ) ?? scopes.get(scope.id)!,
          ).expression(expr, "scope-output." + name),
        ]),
      ),
    };
  });
  const roots = copied.find((scope) => scope.id === scopeId)!;
  const scope = scopeById(target, scopeId),
    layout = { ...target.editor.nodes };
  const rootLayouts = clipboard.selected.map((id) => source.editor.nodes[id]);
  const origin = {
    x: Math.min(...rootLayouts.map((item) => item.x)),
    y: Math.min(...rootLayouts.map((item) => item.y)),
  };
  for (const [oldId, id] of ids) {
    const old = source.editor.nodes[oldId];
    layout[id] = {
      ...old,
      x: old.x + (clipboard.selected.includes(oldId) ? at.x - origin.x : 0),
      y: old.y + (clipboard.selected.includes(oldId) ? at.y - origin.y : 0),
    };
  }
  for (const [key, value] of Object.entries(source.editor.drafts)) {
    const [oldId, ...rest] = key.split(":");
    if (ids.has(oldId)) drafts[ids.get(oldId) + ":" + rest.join(":")] = value;
  }
  let file = replaceScope(target, {
    ...scope,
    edges: [
      ...scope.edges,
      ...roots.edges.filter(
        (edge) =>
          !scope.edges.some(
            (existing) =>
              endpointKey(existing.source) === endpointKey(edge.source) &&
              endpointKey(existing.target) === endpointKey(edge.target),
          ),
      ),
    ],
    nodes: [...scope.nodes, ...roots.nodes],
  });
  file = {
    ...file,
    definition: {
      ...file.definition,
      scopes: [
        ...file.definition.scopes,
        ...copied.filter((item) => item.id !== scopeId),
      ],
    },
    editor: {
      nodes: layout,
      drafts,
      edges: {
        ...target.editor.edges,
        ...Object.fromEntries(
          source.definition.scopes.flatMap((scope) =>
            scope.edges.flatMap((edge) => {
              const id = edgeIds.get(edge.id)!;
              const kept =
                scope.id !== clipboard.sourceScope ||
                file.definition.scopes.some((scope) =>
                  scope.edges.some((edge) => edge.id === id),
                );
              return kept ? [[id, source.editor.edges[edge.id]]] : [];
            }),
          ),
        ),
      },
    },
  };
  return {
    file,
    selected: clipboard.selected.flatMap((id) =>
      ids.has(id) ? [ids.get(id)!] : [],
    ),
  };
}
