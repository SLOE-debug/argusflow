import type { IntentMatchMode } from './actionInspectorTypes';

/** AQL 源码中的半开字符范围。 */
export type AqlSourceRange = Readonly<{ start: number; end: number }>;

/** name 谓词中的字面量或参数绑定。 */
export type ParsedPredicateValue = Readonly<{
  source: 'literal' | 'binding';
  text: string;
  bindingName?: string;
}>;

/** 基础表单可以理解的最终实体查询。 */
export type ParsedTargetEntity = Readonly<{
  role: string;
  roleRange: AqlSourceRange;
  expressionRange: AqlSourceRange;
  argumentsRange: AqlSourceRange;
  namePredicateRange: AqlSourceRange | null;
  nameValue: ParsedPredicateValue;
  match: IntentMatchMode;
  hasMoreConditions: boolean;
}>;

/** CSS 目标及其可安全替换范围。 */
export type ParsedCssTarget = Readonly<{
  expressionRange: AqlSourceRange;
  selectorRange: AqlSourceRange;
  selector: string;
}>;

/** 源码解析结果；无法可逆映射时保持高级查询。 */
export type ParsedAqlTarget =
  | Readonly<{ type: 'entity'; entity: ParsedTargetEntity }>
  | Readonly<{ type: 'css'; css: ParsedCssTarget }>
  | Readonly<{ type: 'advanced' }>;

/** 解析基础表单关注的最终实体；复杂组合查询保持为高级规则。 */
export function parseAqlTarget(source: string): ParsedAqlTarget {
  const expressionRange = resolveTargetExpressionRange(source);
  if (!expressionRange) return { type: 'advanced' };
  const expression = source.slice(expressionRange.start, expressionRange.end);
  const call = parseWholeCall(expression, expressionRange.start);
  if (!call) return { type: 'advanced' };
  if (call.name === 'css') {
    const selectorToken = source.slice(call.argumentsRange.start, call.argumentsRange.end).trim();
    const selectorOffset = source.indexOf(selectorToken, call.argumentsRange.start);
    const selector = parseQuotedString(selectorToken);
    if (selector === null || selectorOffset < 0) return { type: 'advanced' };
    return {
      type: 'css',
      css: {
        expressionRange,
        selectorRange: {
          start: selectorOffset,
          end: selectorOffset + selectorToken.length,
        },
        selector,
      },
    };
  }

  const argumentsWithRanges = splitTopLevelArguments(source, call.argumentsRange);
  const nameArgument = argumentsWithRanges.find(({ text }) => /^\s*name\b/.test(text));
  const otherArguments = argumentsWithRanges.filter(({ text }) => (
    text.trim() && text !== nameArgument?.text
  ));
  const parsedName = nameArgument ? parseNamePredicate(nameArgument.text) : null;
  if (nameArgument && !parsedName) return { type: 'advanced' };
  return {
    type: 'entity',
    entity: {
      role: call.name,
      roleRange: call.nameRange,
      expressionRange,
      argumentsRange: call.argumentsRange,
      namePredicateRange: nameArgument ? trimRange(source, nameArgument.range) : null,
      nameValue: parsedName?.value ?? { source: 'literal', text: '' },
      match: parsedName?.match ?? 'exact',
      hasMoreConditions: otherArguments.length > 0,
    },
  };
}

/** 找到 first/exists/nth/nearest 等外层表达式中的实际目标。 */
function resolveTargetExpressionRange(source: string): AqlSourceRange | null {
  let range = trimRange(source, { start: 0, end: source.length });
  for (let depth = 0; depth < 4; depth += 1) {
    const expression = source.slice(range.start, range.end);
    const call = parseWholeCall(expression, range.start);
    if (!call) return range;
    if (call.name === 'first' || call.name === 'exists' || call.name === 'nth') {
      range = firstTopLevelArgument(source, call.argumentsRange)?.range ?? call.argumentsRange;
      range = trimRange(source, range);
      continue;
    }
    if (call.name === 'nearest') {
      const targetArgument = splitTopLevelArguments(source, call.argumentsRange)
        .find(({ text }) => /^\s*target\s*=/.test(text));
      if (!targetArgument) return null;
      const equalsIndex = source.indexOf('=', targetArgument.range.start);
      if (equalsIndex < 0 || equalsIndex >= targetArgument.range.end) return null;
      range = trimRange(source, { start: equalsIndex + 1, end: targetArgument.range.end });
      continue;
    }
    return range;
  }
  return range;
}

/** 解析一个覆盖完整输入的 AQL 调用，并返回绝对源码范围。 */
function parseWholeCall(
  expression: string,
  absoluteStart: number,
): Readonly<{
  name: string;
  nameRange: AqlSourceRange;
  argumentsRange: AqlSourceRange;
}> | null {
  const match = /^([a-z_][a-z0-9_]*)\s*\(/i.exec(expression);
  if (!match) return null;
  const openIndex = expression.indexOf('(', match[1].length);
  const closeIndex = findMatchingParenthesis(expression, openIndex);
  if (openIndex < 0 || closeIndex !== expression.length - 1) return null;
  return {
    name: match[1],
    nameRange: { start: absoluteStart, end: absoluteStart + match[1].length },
    argumentsRange: {
      start: absoluteStart + openIndex + 1,
      end: absoluteStart + closeIndex,
    },
  };
}

/** 在字符串、正则和嵌套调用之外匹配右括号。 */
function findMatchingParenthesis(source: string, openIndex: number): number {
  let depth = 0;
  let quote: '"' | "'" | null = null;
  let escaped = false;
  let regex = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const character = source[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === '\\') {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) quote = null;
      continue;
    }
    if (regex) {
      if (character === '/') regex = false;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === '/' && mayStartRegex(source, index)) {
      regex = true;
      continue;
    }
    if (character === '(') depth += 1;
    if (character === ')') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return -1;
}

/** 按顶层逗号拆分参数，同时保留每段在原始源码中的范围。 */
function splitTopLevelArguments(
  source: string,
  range: AqlSourceRange,
): ReadonlyArray<Readonly<{ text: string; range: AqlSourceRange }>> {
  const argumentsList: Array<Readonly<{ text: string; range: AqlSourceRange }>> = [];
  let segmentStart = range.start;
  let depth = 0;
  let quote: '"' | "'" | null = null;
  let regex = false;
  let escaped = false;
  for (let index = range.start; index < range.end; index += 1) {
    const character = source[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === '\\') {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) quote = null;
      continue;
    }
    if (regex) {
      if (character === '/') regex = false;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === '/' && mayStartRegex(source, index)) {
      regex = true;
      continue;
    }
    if (character === '(' || character === '[') depth += 1;
    if (character === ')' || character === ']') depth -= 1;
    if (character === ',' && depth === 0) {
      const segmentRange = { start: segmentStart, end: index };
      argumentsList.push({ text: source.slice(segmentStart, index), range: segmentRange });
      segmentStart = index + 1;
    }
  }
  argumentsList.push({
    text: source.slice(segmentStart, range.end),
    range: { start: segmentStart, end: range.end },
  });
  return argumentsList;
}

/** 返回调用的第一个顶层参数。 */
function firstTopLevelArgument(
  source: string,
  range: AqlSourceRange,
): Readonly<{ text: string; range: AqlSourceRange }> | null {
  return splitTopLevelArguments(source, range)[0] ?? null;
}

/** 解析 name 谓词的匹配方式和字面量/参数值。 */
function parseNamePredicate(text: string): Readonly<{
  match: IntentMatchMode;
  value: ParsedPredicateValue;
}> | null {
  const match = /^\s*name\s*(=|contains|starts_with|ends_with|matches)\s*([\s\S]+?)\s*$/.exec(text);
  if (!match) return null;
  const matchMode = operatorToMatchMode(match[1]);
  const token = match[2];
  if (token.startsWith('$')) {
    const bindingName = token.slice(1);
    return /^[a-z_][a-z0-9_]*$/i.test(bindingName)
      ? { match: matchMode, value: { source: 'binding', text: bindingName, bindingName } }
      : null;
  }
  if (matchMode === 'regex') {
    const regexValue = parseRegexLiteral(token);
    return regexValue === null
      ? null
      : { match: matchMode, value: { source: 'literal', text: regexValue } };
  }
  const literal = parseQuotedString(token);
  return literal === null
    ? null
    : { match: matchMode, value: { source: 'literal', text: literal } };
}

/** 把内部操作符映射为属性面板的匹配枚举。 */
function operatorToMatchMode(operator: string): IntentMatchMode {
  switch (operator) {
    case '=':
      return 'exact';
    case 'contains':
      return 'contains';
    case 'starts_with':
      return 'starts_with';
    case 'ends_with':
      return 'ends_with';
    case 'matches':
      return 'regex';
    default:
      return 'exact';
  }
}

/** 解析 JSON 风格双引号字符串。 */
function parseQuotedString(token: string): string | null {
  if (!token.startsWith('"') || !token.endsWith('"')) return null;
  try {
    const value: unknown = JSON.parse(token);
    return typeof value === 'string' ? value : null;
  } catch {
    return null;
  }
}

/** 解析 AQL 正则字面量，保留用户可编辑的 pattern。 */
function parseRegexLiteral(token: string): string | null {
  if (!token.startsWith('/')) return null;
  let escaped = false;
  for (let index = 1; index < token.length; index += 1) {
    const character = token[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === '\\') {
      escaped = true;
      continue;
    }
    if (character === '/') return token.slice(1, index).replaceAll('\\/', '/');
  }
  return null;
}

/** 去除范围两端空白但保留绝对源码偏移。 */
function trimRange(source: string, range: AqlSourceRange): AqlSourceRange {
  let start = range.start;
  let end = range.end;
  while (start < end && /\s/.test(source[start])) start += 1;
  while (end > start && /\s/.test(source[end - 1])) end -= 1;
  return { start, end };
}

/** AQL 中斜杠紧随 matches 时表示正则开始。 */
function mayStartRegex(source: string, index: number): boolean {
  return /matches\s*$/.test(source.slice(Math.max(0, index - 16), index));
}
