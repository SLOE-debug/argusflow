import type { WorkflowFile } from "./contracts";
import { childScopes } from "./factory";
import { scopeById } from "./graph";
/** 名称之外保留声明身份，跨作用域粘贴不会连接到碰巧同名的对象。 */
export function bindingDeclarations(
  file: WorkflowFile,
  scopeId: string,
): Readonly<Record<string, string>> {
  const identities: Record<string, string> = {};
  let id: string | undefined = scopeId;
  const visited = new Set<string>();
  while (id && !visited.has(id)) {
    visited.add(id);
    const scope = scopeById(file, id);
    for (const node of scope.nodes) {
      if (node.action.kind === "let")
        identities["variable/" + node.action.name] ??= node.id;
      if (node.action.kind === "task")
        for (const [port, name] of Object.entries(
          node.action.task.resource_outputs,
        ))
          identities["resource/" + name] ??= node.id + "/" + port;
    }
    const owner = file.definition.scopes
      .flatMap((scope) => scope.nodes.map((node) => ({ scope, node })))
      .find(({ node }) =>
        childScopes(node.action).some((child) => child.id === id),
      );
    if (owner?.node.action.kind === "for_each") {
      identities["variable/" + owner.node.action.item] ??=
        owner.node.id + "/item";
      identities["variable/" + owner.node.action.index] ??=
        owner.node.id + "/index";
    }
    id = owner?.scope.id;
  }
  for (const name of Object.keys(file.definition.resources))
    identities["resource/" + name] ??= file.id + "/resource/" + name;
  return identities;
}
