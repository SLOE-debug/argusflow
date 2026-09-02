import type { AqlQuery, ValueExpr } from '../model/contracts';
import {
  isIntentControlRole,
  type IntentControlRole,
  type IntentMatchMode,
  type IntentTargetValue,
  type QueryTargetIntent,
} from './actionInspectorTypes';
import {
  parseAqlTarget,
  type AqlSourceRange,
  type ParsedPredicateValue,
} from './aqlTargetSource';

/** 把现有 AQL 查询转换成属性面板的目标意图，不改变持久化协议。 */
export function readQueryTargetIntent(query: AqlQuery): QueryTargetIntent {
  const parsed = parseAqlTarget(query.source);
  if (parsed.type === 'css') {
    return { type: 'web', selector: parsed.css.selector, editable: true };
  }
  if (parsed.type === 'advanced') {
    return { type: 'advanced', description: summarizeAdvancedQuery(query.source) };
  }

  const entity = parsed.entity;
  const value = presentPredicateValue(entity.nameValue, query.bindings);
  if (entity.role === 'text') {
    return {
      type: 'text',
      value,
      match: entity.match,
      editable: value.source === 'literal',
      hasMoreConditions: entity.hasMoreConditions,
    };
  }
  if (isIntentControlRole(entity.role)) {
    return {
      type: 'control',
      role: entity.role,
      value,
      match: entity.match,
      editable: value.source === 'literal',
      hasMoreConditions: entity.hasMoreConditions,
    };
  }
  return { type: 'advanced', description: summarizeAdvancedQuery(query.source) };
}

/** 把语义查询切换为文字、控件或网页元素，同时尽量保留外围 AQL 结构。 */
export function changeQueryTargetType(
  query: AqlQuery,
  targetType: 'text' | 'control' | 'web',
): AqlQuery {
  const parsed = parseAqlTarget(query.source);
  if (parsed.type === 'advanced') {
    return { ...query, source: createDefaultTargetSource(targetType), bindings: {} };
  }
  if (targetType === 'web') {
    const expressionRange = parsed.type === 'css'
      ? parsed.css.expressionRange
      : parsed.entity.expressionRange;
    return replaceQuerySourceRange(
      query,
      expressionRange,
      `css(${JSON.stringify('button')})`,
    );
  }
  if (parsed.type === 'css') {
    const role = targetType === 'text' ? 'text' : 'button';
    return replaceQuerySourceRange(
      query,
      parsed.css.expressionRange,
      `${role}(name = ${JSON.stringify('目标')})`,
    );
  }
  const role = targetType === 'text'
    ? 'text'
    : isIntentControlRole(parsed.entity.role) ? parsed.entity.role : 'button';
  return replaceQuerySourceRange(query, parsed.entity.roleRange, role);
}

/** 更新文字、控件名称或 CSS 选择器；参数绑定不会被普通输入框覆盖。 */
export function changeQueryTargetText(query: AqlQuery, text: string): AqlQuery {
  const parsed = parseAqlTarget(query.source);
  if (parsed.type === 'css') {
    return replaceQuerySourceRange(query, parsed.css.selectorRange, JSON.stringify(text));
  }
  if (parsed.type !== 'entity' || parsed.entity.nameValue.source === 'binding') {
    return query;
  }
  const predicate = formatNamePredicate(parsed.entity.match, text);
  if (parsed.entity.namePredicateRange) {
    return replaceQuerySourceRange(query, parsed.entity.namePredicateRange, predicate);
  }
  const argumentsSource = query.source.slice(
    parsed.entity.argumentsRange.start,
    parsed.entity.argumentsRange.end,
  );
  /** 空参数列表直接写入；已有条件则在末尾追加，保留用户原有顺序。 */
  const replacement = argumentsSource.trim()
    ? `${argumentsSource}, ${predicate}`
    : predicate;
  return replaceQuerySourceRange(query, parsed.entity.argumentsRange, replacement);
}

/** 更新基础目标的匹配方式，并保留当前字面量内容。 */
export function changeQueryTargetMatch(
  query: AqlQuery,
  match: IntentMatchMode,
): AqlQuery {
  const parsed = parseAqlTarget(query.source);
  if (
    parsed.type !== 'entity'
    || parsed.entity.nameValue.source === 'binding'
    || !parsed.entity.namePredicateRange
  ) {
    return query;
  }
  return replaceQuerySourceRange(
    query,
    parsed.entity.namePredicateRange,
    formatNamePredicate(match, parsed.entity.nameValue.text),
  );
}

/** 更新控件角色；文字、网页和高级目标不会被该操作改写。 */
export function changeQueryControlRole(
  query: AqlQuery,
  role: IntentControlRole,
): AqlQuery {
  const parsed = parseAqlTarget(query.source);
  if (parsed.type !== 'entity' || !isIntentControlRole(parsed.entity.role)) return query;
  return replaceQuerySourceRange(query, parsed.entity.roleRange, role);
}

/** 返回值表达式的简短用户语言，用于绑定目标和任务摘要。 */
export function formatIntentValueExpression(value: ValueExpr | undefined): string {
  if (!value) return '未绑定的参数';
  switch (value.type) {
    case 'literal':
      return typeof value.value === 'string'
        ? value.value
        : JSON.stringify(value.value);
    case 'ref':
      switch (value.source.type) {
        case 'workflow_input':
          return `流程输入「${value.source.key}」`;
        case 'variable':
          return `变量「${value.source.name}」`;
        case 'node':
          return `节点「${value.source.node_id}」的结果`;
      }
    case 'expression':
      return '表达式结果';
  }
}

/** 把参数引用转换为用户能够理解的值来源。 */
function presentPredicateValue(
  value: ParsedPredicateValue,
  bindings: AqlQuery['bindings'],
): IntentTargetValue {
  if (value.source === 'literal') return value;
  const bindingName = value.bindingName ?? value.text;
  return {
    source: 'binding',
    bindingName,
    text: formatIntentValueExpression(bindings[bindingName]),
  };
}

/** 把用户匹配方式编码回 AQL name 谓词。 */
function formatNamePredicate(match: IntentMatchMode, text: string): string {
  if (match === 'regex') {
    const escapedPattern = text.replaceAll('\\', '\\\\').replaceAll('/', '\\/');
    return `name matches /${escapedPattern}/`;
  }
  const operator = match === 'exact' ? '=' : match;
  return `name ${operator} ${JSON.stringify(text)}`;
}

/** 以不可变方式替换 AQL 源码范围。 */
function replaceQuerySourceRange(
  query: AqlQuery,
  range: AqlSourceRange,
  replacement: string,
): AqlQuery {
  return {
    ...query,
    source: query.source.slice(0, range.start) + replacement + query.source.slice(range.end),
  };
}

/** 创建用户显式切换目标类型时使用的最小语义查询。 */
function createDefaultTargetSource(targetType: 'text' | 'control' | 'web'): string {
  switch (targetType) {
    case 'text':
      return `text(name = ${JSON.stringify('目标文字')})`;
    case 'control':
      return `button(name = ${JSON.stringify('目标')})`;
    case 'web':
      return `css(${JSON.stringify('button')})`;
  }
}

/** 高级查询只展示紧凑摘要，完整源码由 AQL 编辑器负责。 */
function summarizeAdvancedQuery(source: string): string {
  const compact = source.replace(/\s+/g, ' ').trim();
  return compact.length > 72 ? `${compact.slice(0, 69)}…` : compact || '尚未设置查找规则';
}
