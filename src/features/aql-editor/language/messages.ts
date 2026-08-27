import type { AqlDiagnostic, AqlDiagnosticCode } from '../../workflow';

/** AQL 诊断代码对应的默认产品文案。 */
const DIAGNOSTIC_LABELS: Readonly<Record<AqlDiagnosticCode, string>> = {
  empty_query: '请输入查找条件',
  invalid_token: '包含无法识别的内容，或引号、括号没有闭合',
  unexpected_token: '这部分语法不完整',
  unknown_role: '找不到这个控件类型',
  unknown_property: '找不到这个属性',
  unknown_operator: '不支持这个比较方式',
  invalid_predicate: '属性、比较方式和值不匹配',
  invalid_regex: '正则表达式无法使用',
  invalid_argument: '查找函数的参数不正确',
  css_syntax: '请使用 AQL 语法，不要使用 CSS 属性选择器',
  missing_right_parenthesis: '缺少右括号',
  unexpected_right_parenthesis: '多了一个右括号',
  backend_specific_property: '这条规则需要特定的执行方式',
  residual_filter: '这项条件需要额外筛选，执行可能较慢',
  expensive_traversal: '这条规则需要遍历更多元素，执行可能较慢',
  potential_multi_match: '这条规则可能找到多个目标',
  unsupported_backend: '当前方式无法完整执行这条规则',
  runtime_unavailable: '当前运行环境暂不可用',
};

/** Rust 稳定 Hover 代码对应的产品文案。 */
const HOVER_LABELS: Readonly<Record<string, string>> = {
  'aql.hover.role': '控件类型',
  'aql.hover.function': '查找函数',
  'aql.hover.property': '通用属性',
  'aql.hover.backend_property': '特定执行引擎属性；兼容性见运行检查',
  'aql.hover.operator': '比较方式',
  'aql.hover.literal': '固定值',
};

/** 将结构化诊断转换为默认产品文案。 */
export function diagnosticLabel(diagnostic: AqlDiagnostic): string {
  return DIAGNOSTIC_LABELS[diagnostic.code];
}

/** 将 Rust Hover 说明代码转换为产品文案。 */
export function hoverLabel(descriptionCode: string): string {
  return HOVER_LABELS[descriptionCode] ?? descriptionCode;
}
