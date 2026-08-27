import type { AqlDiagnostic, AqlDiagnosticCode } from '../../workflow/contracts';

/** AQL 诊断代码对应的默认产品文案。 */
const DIAGNOSTIC_LABELS: Readonly<Record<AqlDiagnosticCode, string>> = {
  empty_query: '请输入查找规则',
  invalid_token: '包含无法识别或未结束的内容',
  unexpected_token: '这里的语法结构不完整',
  unknown_role: '未知的元素角色',
  unknown_property: '未知的属性',
  unknown_operator: '未知的比较运算符',
  invalid_predicate: '属性、运算符和值的类型不匹配',
  invalid_regex: '正则表达式无效',
  invalid_argument: '查询函数参数无效',
  css_syntax: 'AQL 不使用 CSS 属性选择器语法',
  missing_right_parenthesis: '缺少右括号',
  unexpected_right_parenthesis: '存在多余的右括号',
  backend_specific_property: '这条规则使用了后端专用属性',
  residual_filter: '这个条件需要额外筛选，执行可能稍慢',
  expensive_traversal: '这条规则需要额外遍历',
  potential_multi_match: '这条规则可能找到多个目标',
  unsupported_backend: '该执行方式不能完整保持查询语义',
  runtime_unavailable: '当前执行环境不可用',
};

/** Rust 稳定 Hover 代码对应的产品文案。 */
const HOVER_LABELS: Readonly<Record<string, string>> = {
  'aql.hover.role': '元素语义角色',
  'aql.hover.function': 'AQL 查询函数',
  'aql.hover.property': '跨后端属性',
  'aql.hover.backend_property': '后端专用属性；兼容性由执行预览提供',
  'aql.hover.operator': 'AQL 比较运算符',
  'aql.hover.literal': 'AQL 字面量',
};

/** 将结构化诊断转换为默认产品文案。 */
export function diagnosticLabel(diagnostic: AqlDiagnostic): string {
  return DIAGNOSTIC_LABELS[diagnostic.code];
}

/** 将 Rust Hover 说明代码转换为产品文案。 */
export function hoverLabel(descriptionCode: string): string {
  return HOVER_LABELS[descriptionCode] ?? descriptionCode;
}
