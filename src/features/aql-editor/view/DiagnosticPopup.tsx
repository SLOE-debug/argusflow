import { AlertTriangle } from 'lucide-react';

import type { AqlDiagnostic, AqlDiagnosticCode } from '../../workflow/contracts';

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

/** 展示第一个高优先级诊断；完整列表仍保留在 decoration 与 Explain 中。 */
export function DiagnosticPopup({
  diagnostics,
}: Readonly<{ diagnostics: readonly AqlDiagnostic[] }>) {
  const diagnostic = diagnostics.find((candidate) => candidate.severity === 'error')
    ?? diagnostics[0];
  if (!diagnostic) {
    return null;
  }
  const position = diagnostic.range?.start;

  return (
    <div
      className={diagnostic.severity === 'error'
        ? 'flex items-start gap-1.5 rounded-md border border-rose-200 bg-rose-50 px-2.5 py-2 text-[10px] leading-4 text-rose-700'
        : 'flex items-start gap-1.5 rounded-md border border-amber-200 bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700'}
      role={diagnostic.severity === 'error' ? 'alert' : 'status'}
    >
      <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
      <span>
        {position ? `第 ${position.line + 1} 行，第 ${position.utf16_column + 1} 列：` : ''}
        {DIAGNOSTIC_LABELS[diagnostic.code]}
      </span>
    </div>
  );
}

/** 将结构化诊断代码转换为默认产品文案。 */
export function diagnosticLabel(diagnostic: AqlDiagnostic): string {
  return DIAGNOSTIC_LABELS[diagnostic.code];
}
