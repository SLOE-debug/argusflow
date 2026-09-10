import type * as Monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import type { LanguageService, SymbolKind } from '../../../features/aql';
import { completionContent, hoverContent } from './presentation';

export const LANGUAGE_ID = 'argusflow-aql';
/** 对接 Monaco 的局部注册会随编辑器卸载回收。 */
export function registerLanguage(monaco: typeof Monaco, service: LanguageService, isComposing: () => boolean = () => false): Monaco.IDisposable {
  const kinds: Record<SymbolKind, Monaco.languages.CompletionItemKind> = {
    Role: monaco.languages.CompletionItemKind.Class,
    Function: monaco.languages.CompletionItemKind.Function,
    Attribute: monaco.languages.CompletionItemKind.Property,
    Operator: monaco.languages.CompletionItemKind.Operator,
    Boolean: monaco.languages.CompletionItemKind.Value,
    Parameter: monaco.languages.CompletionItemKind.Variable,
  };
  const disposables = [
    monaco.languages.register({ id: LANGUAGE_ID }),
    monaco.languages.setLanguageConfiguration(LANGUAGE_ID, {
      brackets: [['(', ')']],
      autoClosingPairs: [{ open: '(', close: ')' }, { open: '"', close: '"' }],
      comments: { lineComment: '//', blockComment: ['/*', '*/'] },
      wordPattern: /[^\s(),=<>!"/]+/,
    }),
    monaco.languages.setMonarchTokensProvider(LANGUAGE_ID, {
      tokenizer: { root: [
        [/\/\/.*$/, 'comment'],
        [/\/\*/, 'comment', '@comment'],
        [/"(?:\\.|[^"\\])*"?/, 'string'],
        [/\/(?:\\.|[^/\\])+\/[a-z]*/, 'regexp'],
        [/\$[^\s(),=<>!"/]+/, 'variable'],
        [/[(),]/, 'delimiter'],
        [/[=<>!]+/, 'operator'],
        [/[\d.]+/, 'number'],
        [/[^\s(),=<>!"/]+/, 'keyword'],
      ], comment: [[/[^*]+/, 'comment'], [/\*\//, 'comment', '@pop'], [/\*/, 'comment']] },
    }),
    monaco.languages.registerCompletionItemProvider(LANGUAGE_ID, {
      triggerCharacters: ['(', ',', '.', '$'],
      provideCompletionItems: (model, position) => ({
        // 输入后重新请求可按新的中英文前缀检索，不沿用上一前缀的候选集合。
        incomplete: true,
        suggestions: (isComposing() ? [] : service.completions(model.getValue(), { line: position.lineNumber - 1, column: position.column - 1 }))
          .map((item) => completionContent(item, kinds[item.kind], monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet)),
      }),
    }),
    monaco.languages.registerHoverProvider(LANGUAGE_ID, {
      provideHover: (model, position) => {
        const item = service.hover(model.getValue(), { line: position.lineNumber - 1, column: position.column - 1 });
        return item ? hoverContent(item) : undefined;
      },
    }),
    monaco.languages.registerDocumentFormattingEditProvider(LANGUAGE_ID, {
      provideDocumentFormattingEdits: (model) => {
        if (isComposing()) return [];
        const formatted = service.inspect(model.getValue()).formatted;
        return formatted === undefined ? [] : [{ range: model.getFullModelRange(), text: formatted }];
      },
    }),
  ];
  return { dispose: () => disposables.forEach((item) => item?.dispose()) };
}
