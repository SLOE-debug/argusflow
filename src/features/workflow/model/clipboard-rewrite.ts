import type { Action, Expr } from "./contracts";
/** 每个字段独立收集失效引用，重新配置后可独立解除草稿。 */
export interface ReferenceRewriter {
  expression(value: Expr, field: string): Expr;
  resource(value: string, field: string): string;
  variable(value: string, field: string): string;
  scope(id: string): string;
}
export function rewriteExpression(
  expr: Expr,
  nodeIds: ReadonlyMap<string, string>,
  variable: (name: string) => string,
  external: (value: Expr) => void,
): Expr {
  const recur = (value: Expr) =>
    rewriteExpression(value, nodeIds, variable, external);
  switch (expr.kind) {
    case "variable":
      return { ...expr, name: variable(expr.name) };
    case "node_output": {
      const id = nodeIds.get(expr.node);
      if (!id) external(expr);
      return { ...expr, node: id ?? expr.node };
    }
    case "input":
      external(expr);
      return expr;
    case "binary":
      return { ...expr, left: recur(expr.left), right: recur(expr.right) };
    case "choose":
      return {
        ...expr,
        condition: recur(expr.condition),
        then_value: recur(expr.then_value),
        else_value: recur(expr.else_value),
      };
    case "field":
    case "not":
    case "some":
      return { ...expr, value: recur(expr.value) };
    case "index":
      return { ...expr, value: recur(expr.value), index: recur(expr.index) };
    case "function":
      return { ...expr, arguments: expr.arguments.map(recur) };
    case "list":
      return { ...expr, items: expr.items.map(recur) };
    case "record":
      return {
        ...expr,
        fields: Object.fromEntries(
          Object.entries(expr.fields).map(([key, value]) => [
            key,
            recur(value),
          ]),
        ),
      };
    case "literal":
    case "result":
      return expr;
  }
}
/** 按当前 Action 联合逐项重写，配置里的任意 JSON 不参与图引用改写。 */
export function rewriteAction(
  action: Action,
  rewrite: ReferenceRewriter,
): Action {
  const inputs = (values: Readonly<Record<string, Expr>>) =>
    Object.fromEntries(
      Object.entries(values).map(([key, value]) => [
        key,
        rewrite.expression(value, "input." + key),
      ]),
    );
  const resources = (values: Readonly<Record<string, string>>) =>
    Object.fromEntries(
      Object.entries(values).map(([key, value]) => [
        key,
        rewrite.resource(value, "resource." + key),
      ]),
    );
  switch (action.kind) {
    case "let":
      return {
        ...action,
        name: rewrite.variable(action.name, "name"),
        value: rewrite.expression(action.value, "value"),
      };
    case "assign":
      return {
        ...action,
        assignments: action.assignments.map((item, index) => ({
          name: rewrite.variable(item.name, "assignment." + index),
          value: rewrite.expression(item.value, "assignment." + index),
        })),
      };
    case "block":
      return { ...action, scope: rewrite.scope(action.scope) };
    case "if":
      return {
        ...action,
        condition: rewrite.expression(action.condition, "condition"),
        then_scope: rewrite.scope(action.then_scope),
        else_scope: rewrite.scope(action.else_scope),
      };
    case "switch":
      return {
        ...action,
        selector: rewrite.expression(action.selector, "selector"),
        cases: action.cases.map((item) => ({
          ...item,
          scope: rewrite.scope(item.scope),
        })),
        default_scope: rewrite.scope(action.default_scope),
      };
    case "while":
      return {
        ...action,
        body: rewrite.scope(action.body),
        condition: rewrite.expression(action.condition, "condition"),
      };
    case "for_each":
      return {
        ...action,
        body: rewrite.scope(action.body),
        items: rewrite.expression(action.items, "items"),
      };
    case "call":
    case "call_workflow":
      return {
        ...action,
        inputs: inputs(action.inputs),
        resources: resources(action.resources),
      };
    case "return":
      return {
        ...action,
        values: Object.fromEntries(
          Object.entries(action.values).map(([key, value]) => [
            key,
            rewrite.expression(value, "return." + key),
          ]),
        ),
      };
    case "wait":
      return {
        ...action,
        milliseconds: rewrite.expression(action.milliseconds, "milliseconds"),
      };
    case "release":
      return {
        ...action,
        resource: rewrite.resource(action.resource, "resource"),
      };
    case "task":
      return {
        ...action,
        task: {
          ...action.task,
          inputs: inputs(action.task.inputs),
          resources: resources(action.task.resources),
          resource_outputs: Object.fromEntries(
            Object.entries(action.task.resource_outputs).map(([port, name]) => [
              port,
              rewrite.resource(name, "created." + port),
            ]),
          ),
        },
      };
    case "break":
    case "continue":
    case "fail":
      return action;
  }
}
