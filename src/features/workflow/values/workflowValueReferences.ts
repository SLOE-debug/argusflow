import type {
  ValueExpr,
} from '../model/contracts';
import type {
  WorkflowCanvasNode,
  WorkflowNodeData,
} from '../model/workflowModel';

/** 可被结构化重命名事务更新的工作流值来源。 */
export type WorkflowReferenceKind = 'workflow_input' | 'variable';

/** 一次输入或变量重命名需要同步的稳定引用描述。 */
export type WorkflowReferenceRename = Readonly<{
  /** 引用来源类别。 */
  kind: WorkflowReferenceKind;
  /** 重命名前的声明名称。 */
  oldName: string;
  /** 重命名后的声明名称。 */
  newName: string;
}>;

/** 不能由结构化重命名事务安全改写的高级表达式引用位置。 */
export type WorkflowExpressionReferenceLocation = Readonly<{
  /** 表达式所属节点的稳定 ID。 */
  nodeId: string;
  /** 表达式所属节点的当前显示名称。 */
  nodeLabel: string;
}>;

/** 把结构化 ValueExpr 引用改写到新的输入或变量名称。 */
export function renameValueReference(
  expression: ValueExpr,
  rename: WorkflowReferenceRename,
): ValueExpr {
  if (expression.type !== 'ref') return expression;
  if (rename.kind === 'workflow_input'
    && expression.source.type === 'workflow_input'
    && expression.source.key === rename.oldName) {
    return {
      ...expression,
      source: { ...expression.source, key: rename.newName },
    };
  }
  if (rename.kind === 'variable'
    && expression.source.type === 'variable'
    && expression.source.name === rename.oldName) {
    return {
      ...expression,
      source: { ...expression.source, name: rename.newName },
    };
  }
  return expression;
}

/**
 * 在一批画布节点中同步结构化引用。
 *
 * 高级表达式保留为源码字符串，不做不安全的全文替换；所有结构化 ValueExpr、
 * Set Variables 目标和公开 output binding 都在同一次文档事务中更新。
 */
export function renameWorkflowReferences(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  rename: WorkflowReferenceRename,
): WorkflowCanvasNode[] {
  return nodes.map((node) => ({
    ...node,
    data: renameNodeReferences(node.data, rename),
  }));
}

/** 判断结构化节点数据是否引用指定输入或变量。 */
export function countWorkflowReferences(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  kind: WorkflowReferenceKind,
  name: string,
): number {
  return nodes.reduce((count, node) => count + countNodeReferences(node.data, kind, name), 0);
}

/** 查找直接读取目标声明的 Rhai 表达式，供删除和重命名操作提前阻止断链。 */
export function findExpressionReferenceLocations(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  kind: WorkflowReferenceKind,
  name: string,
): ReadonlyArray<WorkflowExpressionReferenceLocation> {
  return nodes.flatMap((node) => nodeContainsExpressionReference(node.data, kind, name)
    ? [{ nodeId: node.id, nodeLabel: node.data.label }]
    : []);
}

/** 将一个节点的所有已知值字段映射到新的声明名称。 */
function renameNodeReferences(
  data: WorkflowNodeData,
  rename: WorkflowReferenceRename,
): WorkflowNodeData {
  if (data.kind !== 'component') {
    const rewritten = rewriteValue(
      data,
      (expression) => renameValueReference(expression, rename),
    );
    return rewritten.kind === 'variable' && rename.kind === 'variable'
      ? {
          ...rewritten,
          assignments: rewritten.assignments.map((assignment) => (
            assignment.name === rename.oldName
              ? { ...assignment, name: rename.newName }
              : assignment
          )),
        }
      : rewritten;
  }
  /** 组件定义是版本锁定的内部工作流，不能被外层声明改名事务修改。 */
  const { componentDefinition, componentOutputs, ...mutableData } = data;
  const rewritten = rewriteValue(
    mutableData,
    (expression) => renameValueReference(expression, rename),
  );
  return { ...rewritten, componentDefinition, componentOutputs };
}

/** 统计一个节点中结构化引用和 Set Variables 目标的出现次数。 */
function countNodeReferences(
  data: WorkflowNodeData,
  kind: WorkflowReferenceKind,
  name: string,
): number {
  let count = 0;
  const visit = (expression: ValueExpr) => {
    count += countValueReference(expression, kind, name);
  };
  if (data.kind === 'component') {
    const {
      componentDefinition: _componentDefinition,
      componentOutputs: _componentOutputs,
      ...mutableData
    } = data;
    visitValueExpressions(mutableData, visit);
  } else {
    visitValueExpressions(data, visit);
  }
  if (data.kind === 'variable' && kind === 'variable') {
    count += data.assignments.filter((assignment) => assignment.name === name).length;
  }
  return count;
}

/** 统计单个 ValueExpr 是否指向目标声明。 */
function countValueReference(
  expression: ValueExpr,
  kind: WorkflowReferenceKind,
  name: string,
): number {
  if (expression.type === 'expression') {
    return isDirectExpressionReference(expression.source, kind, name) ? 1 : 0;
  }
  if (expression.type !== 'ref') return 0;
  return kind === 'workflow_input'
    ? expression.source.type === 'workflow_input' && expression.source.key === name ? 1 : 0
    : expression.source.type === 'variable' && expression.source.name === name ? 1 : 0;
}

/** 判断节点的任一高级表达式是否直接使用目标输入或变量。 */
function nodeContainsExpressionReference(
  data: WorkflowNodeData,
  kind: WorkflowReferenceKind,
  name: string,
): boolean {
  let found = false;
  const visit = (expression: ValueExpr) => {
    if (expression.type === 'expression'
      && isDirectExpressionReference(expression.source, kind, name)) {
      found = true;
    }
  };
  if (data.kind === 'component') {
    const {
      componentDefinition: _componentDefinition,
      componentOutputs: _componentOutputs,
      ...mutableData
    } = data;
    visitValueExpressions(mutableData, visit);
  } else {
    visitValueExpressions(data, visit);
  }
  return found;
}

/**
 * 识别编辑器正式生成的 `root["name"]` 以及标识符可用时的 `root.name`。
 *
 * 这里只用于拒绝不安全重命名，不尝试重写源码；允许空白可覆盖用户格式化后的等价写法。
 */
function isDirectExpressionReference(
  source: string,
  kind: WorkflowReferenceKind,
  name: string,
): boolean {
  const root = kind === 'workflow_input' ? 'input' : 'vars';
  const doubleQuotedName = escapeRegularExpression(JSON.stringify(name).slice(1, -1));
  const bracketReference = new RegExp(
    `(?:^|[^\\w])${root}\\s*\\[\\s*"${doubleQuotedName}"\\s*\\]`,
  );
  if (bracketReference.test(source)) return true;
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return false;
  const propertyReference = new RegExp(
    `(?:^|[^\\w])${root}\\s*\\.\\s*${escapeRegularExpression(name)}(?:$|[^\\w])`,
  );
  return propertyReference.test(source);
}

/** 把声明名安全嵌入检测用正则。 */
function escapeRegularExpression(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** 遍历未知节点数据中的所有结构化 ValueExpr；高级表达式作为不可变源码节点处理。 */
function visitValueExpressions(
  value: unknown,
  visit: (expression: ValueExpr) => void,
): void {
  if (!value || typeof value !== 'object') return;
  if (isValueExpr(value)) {
    visit(value);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((child) => visitValueExpressions(child, visit));
    return;
  }
  Object.values(value).forEach((child) => visitValueExpressions(child, visit));
}

/** 递归复制节点数据并只改写结构完整的 ValueExpr。 */
function rewriteValue<T>(
  value: T,
  rewrite: (expression: ValueExpr) => ValueExpr,
): T {
  if (!value || typeof value !== 'object') return value;
  if (isValueExpr(value)) return rewrite(value) as T;
  if (Array.isArray(value)) {
    return value.map((child) => rewriteValue(child, rewrite)) as T;
  }
  return Object.fromEntries(Object.entries(value).map(([key, child]) => (
    [key, rewriteValue(child, rewrite)]
  ))) as T;
}

/** 使用判别字段识别一个完整 ValueExpr，避免碰到普通业务对象。 */
function isValueExpr(value: object): value is ValueExpr {
  const candidate = value as Partial<ValueExpr>;
  return candidate.type === 'literal'
    || candidate.type === 'expression'
    || (candidate.type === 'ref' && 'source' in candidate);
}
