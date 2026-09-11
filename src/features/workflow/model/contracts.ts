/** 当前编排文档的无损传输契约；整数值不经过 JavaScript number。 */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
export type ValueType =
  | { readonly type: "bool" | "int" | "float" | "text" }
  | { readonly type: "list" | "optional"; readonly of: ValueType }
  | {
      readonly type: "record";
      readonly of: Readonly<Record<string, ValueType>>;
    };
export type Value =
  | { readonly type: "bool"; readonly value: boolean }
  | { readonly type: "int"; readonly value: string }
  | { readonly type: "float"; readonly value: number }
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "list"; readonly value: readonly Value[] }
  | { readonly type: "record"; readonly value: Readonly<Record<string, Value>> }
  | { readonly type: "optional"; readonly value: Value | null };
export const BINARY_OPS = [
  "add",
  "subtract",
  "multiply",
  "divide",
  "remainder",
  "equal",
  "not_equal",
  "less",
  "less_equal",
  "greater",
  "greater_equal",
  "and",
  "or",
] as const;
export type BinaryOp = (typeof BINARY_OPS)[number];
export const FUNCTIONS = [
  "length",
  "concat",
  "contains",
  "trim",
  "lowercase",
  "uppercase",
  "split",
  "join",
  "append",
  "is_some",
  "or_else",
  "to_float",
  "to_text",
] as const;
export type Expr =
  | {
      readonly kind: "literal";
      readonly value_type: ValueType;
      readonly value: Value;
    }
  | { readonly kind: "variable" | "input"; readonly name: string }
  | {
      readonly kind: "node_output";
      readonly node: string;
      readonly output: string;
    }
  | { readonly kind: "result"; readonly output: string }
  | { readonly kind: "field"; readonly value: Expr; readonly field: string }
  | { readonly kind: "index"; readonly value: Expr; readonly index: Expr }
  | {
      readonly kind: "binary";
      readonly op: BinaryOp;
      readonly left: Expr;
      readonly right: Expr;
    }
  | { readonly kind: "not" | "some"; readonly value: Expr }
  | {
      readonly kind: "choose";
      readonly condition: Expr;
      readonly then_value: Expr;
      readonly else_value: Expr;
    }
  | { readonly kind: "record"; readonly fields: Readonly<Record<string, Expr>> }
  | {
      readonly kind: "list";
      readonly item_type: ValueType;
      readonly items: readonly Expr[];
    }
  | {
      readonly kind: "function";
      readonly function: (typeof FUNCTIONS)[number];
      readonly arguments: readonly Expr[];
    };
export interface TaskDefinition {
  readonly type_id: string;
  readonly version: 1;
  readonly config: Readonly<Record<string, JsonValue>>;
  readonly inputs: Readonly<Record<string, Expr>>;
  readonly resources: Readonly<Record<string, string>>;
  readonly resource_outputs: Readonly<Record<string, string>>;
  readonly retry: null;
}
export type Action =
  | {
      readonly kind: "let";
      readonly name: string;
      readonly value_type: ValueType;
      readonly value: Expr;
    }
  | {
      readonly kind: "assign";
      readonly assignments: readonly {
        readonly name: string;
        readonly value: Expr;
      }[];
    }
  | { readonly kind: "block"; readonly scope: string }
  | {
      readonly kind: "if";
      readonly condition: Expr;
      readonly then_scope: string;
      readonly else_scope: string;
    }
  | {
      readonly kind: "switch";
      readonly selector: Expr;
      readonly cases: readonly {
        readonly value: Value;
        readonly scope: string;
      }[];
      readonly default_scope: string;
    }
  | {
      readonly kind: "while";
      readonly condition: Expr;
      readonly body: string;
      readonly max_iterations: number | null;
    }
  | {
      readonly kind: "for_each";
      readonly items: Expr;
      readonly item: string;
      readonly index: string;
      readonly body: string;
      readonly max_iterations: number | null;
    }
  | { readonly kind: "break" | "continue" }
  | {
      readonly kind: "call";
      readonly subflow: string;
      readonly inputs: Readonly<Record<string, Expr>>;
      readonly resources: Readonly<Record<string, string>>;
    }
  | {
      readonly kind: "call_workflow";
      readonly workflow: string;
      readonly inputs: Readonly<Record<string, Expr>>;
      readonly resources: Readonly<Record<string, string>>;
    }
  | { readonly kind: "return"; readonly values: Readonly<Record<string, Expr>> }
  | { readonly kind: "fail"; readonly code: string }
  | { readonly kind: "wait"; readonly milliseconds: Expr }
  | { readonly kind: "release"; readonly resource: string }
  | { readonly kind: "task"; readonly task: TaskDefinition };
export interface WorkflowNode {
  readonly id: string;
  readonly next: string | null;
  readonly timeout_ms: string | null;
  readonly action: Action;
  readonly output_bindings: Readonly<Record<string, Expr>>;
}
export interface Scope {
  readonly id: string;
  readonly entry: string | null;
  readonly nodes: readonly WorkflowNode[];
  readonly outputs: Readonly<Record<string, Expr>>;
}
export interface WorkflowDefinition {
  readonly schema_version: 1;
  readonly name: string;
  readonly inputs: Readonly<Record<string, ValueType>>;
  readonly outputs: Readonly<Record<string, ValueType>>;
  readonly resources: Readonly<Record<string, string>>;
  readonly root: string;
  readonly scopes: readonly Scope[];
  readonly subflows: Readonly<
    Record<
      string,
      {
        readonly scope: string;
        readonly inputs: Readonly<Record<string, ValueType>>;
        readonly outputs: Readonly<Record<string, ValueType>>;
        readonly resources: Readonly<Record<string, string>>;
      }
    >
  >;
}
export interface NodeLayout {
  readonly x: number;
  readonly y: number;
  readonly label: string;
  readonly note: string;
}
export interface WorkflowFile {
  readonly format_version: 1;
  readonly id: string;
  readonly definition: WorkflowDefinition;
  readonly editor: {
    readonly nodes: Readonly<Record<string, NodeLayout>>;
    /** 未通过字段解析的原文也保存；编译时必须先消除这些草稿。 */
    readonly drafts: Readonly<Record<string, string>>;
  };
}
export interface Problem {
  readonly code: string;
  readonly message: string;
  readonly workflow?: string;
  readonly scope: string | null;
  readonly node: string | null;
  readonly field?: string;
}
export interface DocumentSummary {
  readonly id: string;
  readonly name: string;
  readonly revision: string;
}
