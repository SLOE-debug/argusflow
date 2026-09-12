import type {
  Expr,
  ValueType,
  WorkflowFile,
  WorkflowNode,
} from "../model/contracts";
import { childScopes } from "../model/factory";
import { scopeById, nodeTitle } from "../model/graph";
import { taskSpec } from "../nodes/catalog";
import { graphIndex } from "../model/connections";
export interface SymbolValue {
  readonly label: string;
  readonly expression: Expr;
  readonly type: ValueType;
}
export interface ResourceValue {
  readonly name: string;
  readonly type: string;
}
/** 候选只来自已经执行的同域步骤与可见祖先，运行前仍由 Rust 做权威校验。 */
export function availableSymbols(
  file: WorkflowFile,
  scopeId: string,
  before?: string,
  documents: readonly WorkflowFile[] = [],
): {
  readonly values: readonly SymbolValue[];
  readonly resources: readonly ResourceValue[];
} {
  const values = new Map<string, SymbolValue>();
  const resources = new Map<string, ResourceValue>();
  Object.entries(file.definition.inputs).forEach(([name, type]) =>
    values.set("input:" + name, {
      label: "输入 · " + name,
      expression: { kind: "input", name },
      type,
    }),
  );
  Object.entries(file.definition.resources).forEach(([name, type]) =>
    resources.set(name, { name, type }),
  );
  const chain: { scope: string; before?: string; owner?: WorkflowNode }[] = [
    { scope: scopeId, before },
  ];
  let child = scopeId;
  while (child !== file.definition.root) {
    const parent = file.definition.scopes
      .flatMap((scope) => scope.nodes.map((node) => ({ scope, node })))
      .find(({ node }) =>
        childScopes(node.action).some((item) => item.id === child),
      );
    if (!parent) break;
    chain[0].owner = parent.node;
    chain.unshift({ scope: parent.scope.id, before: parent.node.id });
    child = parent.scope.id;
  }
  for (const item of chain) {
    const scope = scopeById(file, item.scope);
    // 同域声明遮蔽祖先，即使尚未初始化也不能回退同名变量。
    scope.nodes.forEach((node) => {
      if (node.action.kind === "let") values.delete("var:" + node.action.name);
    });
    const owner = item.owner;
    if (owner?.action.kind === "for_each") {
      const type = inferExpression(owner.action.items, [...values.values()]);
      if (type?.type === "list")
        values.set("var:" + owner.action.item, {
          label: "当前项 · " + owner.action.item,
          expression: { kind: "variable", name: owner.action.item },
          type: type.of,
        });
      values.set("var:" + owner.action.index, {
        label: "索引 · " + owner.action.index,
        expression: { kind: "variable", name: owner.action.index },
        type: { type: "int" },
      });
    }
    const prefix = graphIndex(scope).prefix();
    // 孤立节点和分叉之后的节点没有确定执行次序，不假定前缀已执行。
    for (const cursor of item.before && !prefix.includes(item.before)
      ? []
      : prefix) {
      if (cursor === item.before) break;
      const node = scope.nodes.find((node) => node.id === cursor);
      if (!node) break;
      const action = node.action;
      if (action.kind === "let")
        values.set("var:" + action.name, {
          label: "变量 · " + action.name,
          expression: { kind: "variable", name: action.name },
          type: action.value_type,
        });
      if (action.kind === "task") {
        const spec = taskSpec(action.task.type_id);
        for (const [port, name] of Object.entries(action.task.resource_outputs))
          if (spec?.creates[port])
            resources.set(name, { name, type: spec.creates[port] });
        for (const [output, type] of Object.entries(spec?.outputs ?? {}))
          values.set(node.id + ":" + output, {
            label: nodeTitle(file, node) + " · " + output,
            expression: { kind: "node_output", node: node.id, output },
            type,
          });
      }
      if (action.kind === "call_workflow") {
        const target = documents.find((doc) => doc.id === action.workflow);
        for (const [output, type] of Object.entries(
          target?.definition.outputs ?? {},
        ))
          values.set(node.id + ":" + output, {
            label: nodeTitle(file, node) + " · " + output,
            expression: { kind: "node_output", node: node.id, output },
            type,
          });
      }
      if (
        action.kind === "block" ||
        action.kind === "if" ||
        action.kind === "switch"
      ) {
        const first = childScopes(action).find(
          (child) => Object.keys(scopeById(file, child.id).outputs).length,
        );
        if (first) {
          const inside = availableSymbols(file, first.id, undefined, documents);
          for (const [output, expr] of Object.entries(
            scopeById(file, first.id).outputs,
          )) {
            const type = inferExpression(expr, inside.values);
            if (type)
              values.set(node.id + ":" + output, {
                label: nodeTitle(file, node) + " · " + output,
                expression: { kind: "node_output", node: node.id, output },
                type,
              });
          }
        }
      }
      for (const [output, expr] of Object.entries(node.output_bindings)) {
        const type = inferExpression(expr, [...values.values()]);
        if (type)
          values.set(node.id + ":" + output, {
            label: nodeTitle(file, node) + " · " + output,
            expression: { kind: "node_output", node: node.id, output },
            type,
          });
      }
      if (action.kind === "release") resources.delete(action.resource);
    }
  }
  return { values: [...values.values()], resources: [...resources.values()] };
}
export function inferExpression(
  expr: Expr,
  symbols: readonly SymbolValue[],
): ValueType | undefined {
  if (expr.kind === "literal") return expr.value_type;
  if (expr.kind === "list") return { type: "list", of: expr.item_type };
  if (expr.kind === "binary")
    return [
      "equal",
      "not_equal",
      "less",
      "less_equal",
      "greater",
      "greater_equal",
      "and",
      "or",
    ].includes(expr.op)
      ? { type: "bool" }
      : inferExpression(expr.left, symbols);
  if (expr.kind === "not") return { type: "bool" };
  if (expr.kind === "record") {
    const fields = Object.entries(expr.fields).map(
      ([name, value]) => [name, inferExpression(value, symbols)] as const,
    );
    if (fields.every((field) => field[1]))
      return {
        type: "record",
        of: Object.fromEntries(fields) as Record<string, ValueType>,
      };
  }
  if (expr.kind === "field") {
    const type = inferExpression(expr.value, symbols);
    return type?.type === "record" ? type.of[expr.field] : undefined;
  }
  if (expr.kind === "index") {
    const type = inferExpression(expr.value, symbols);
    return type?.type === "list" ? type.of : undefined;
  }
  return symbols.find(
    (item) => JSON.stringify(item.expression) === JSON.stringify(expr),
  )?.type;
}
