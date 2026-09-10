import type * as Monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import type { Completion, Hover, Range, SymbolKind } from '../../../features/aql';

/** Monaco 和编辑语言服务的类别名称只在展示边界转换。 */
const KIND_LABELS: Readonly<Record<SymbolKind, string>> = {
  Role: '角色', Function: '函数', Attribute: '属性', Operator: '运算符', Boolean: '布尔值', Parameter: '参数',
};

/** WASM 使用零基行列；Monaco 使用一基行列，列单位均为 UTF-16。 */
export function editorRange(range: Range): Monaco.IRange {
  return {
    startLineNumber: range.start.line + 1, startColumn: range.start.column + 1,
    endLineNumber: range.end.line + 1, endColumn: range.end.column + 1,
  };
}

/** 使用代码块展示签名和示例，禁用 HTML 与受信命令链接。 */
export function hoverContent(item: Hover): Monaco.languages.Hover {
  return {
    range: editorRange(item.range),
    contents: [
      { value: `${KIND_LABELS[item.kind]} · ${item.title}`, isTrusted: false, supportHtml: false },
      { value: `\`\`\`text\n${item.signature}\n\`\`\``, isTrusted: false, supportHtml: false },
      { value: item.description, isTrusted: false, supportHtml: false },
      { value: `示例\n\n\`\`\`text\n${item.example}\n\`\`\``, isTrusted: false, supportHtml: false },
    ],
  };
}

/** 中文显示词与用于英文输入的筛选词分开，避免 Monaco 再次筛选时隐藏候选。 */
export function completionContent(item: Completion, kind: Monaco.languages.CompletionItemKind, snippetRule: Monaco.languages.CompletionItemInsertTextRule): Monaco.languages.CompletionItem {
  return {
    label: item.label, filterText: item.filter_text, insertText: item.insert_text,
    insertTextRules: item.insert_as_snippet ? snippetRule : undefined,
    range: editorRange(item.range), kind,
    detail: `${KIND_LABELS[item.kind]} · ${item.detail}`,
    documentation: { value: `${item.description}\n\n\`\`\`text\n${item.example}\n\`\`\``, isTrusted: false, supportHtml: false },
  };
}
