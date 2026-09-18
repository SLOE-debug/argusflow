import type { EditorLanguage } from "../../editor/monaco";
import {
  FUNCTIONS,
  formatExpression,
  type SymbolValue,
} from "../../../features/workflow";
/** 每个模型使用独立语言身份，候选不会泄漏其他节点的变量作用域。 */
export function expressionLanguage(
  id: string,
  symbols: () => readonly SymbolValue[],
): EditorLanguage {
  return {
    id,
    register(api, target, composing) {
      const disposables = [
        api.languages.register({ id }),
        api.languages.setLanguageConfiguration(id, {
          brackets: [
            ["(", ")"],
            ["[", "]"],
            ["{", "}"],
          ],
          autoClosingPairs: [
            { open: "(", close: ")" },
            { open: "[", close: "]" },
            { open: "{", close: "}" },
            { open: '"', close: '"' },
          ],
        }),
        api.languages.setMonarchTokensProvider(id, {
          tokenizer: {
            root: [
              [/"(?:\\.|[^"\\])*"?|'(?:\\.|[^'\\])*'?/, "string"],
              [/\b(?:true|false|null)\b/, "keyword"],
              [/\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/, "number"],
              [/[+*/%=!<>?:&|\-]+/, "operator"],
              [/[\p{L}_$][\p{L}\p{N}_$]*/u, "variable"],
            ],
          },
        }),
        api.languages.registerCompletionItemProvider(id, {
          triggerCharacters: ["."],
          provideCompletionItems(model, position) {
            if (model !== target || composing()) return { suggestions: [] };
            const word = model.getWordUntilPosition(position);
            const prefix = model
              .getLineContent(position.lineNumber)
              .slice(0, position.column - 1);
            const reference =
              prefix.match(
                /(?:[\p{L}_$][\p{L}\p{N}_$]*\.)*[\p{L}\p{N}_$]*$/u,
              )?.[0] ?? word.word;
            const range = {
              startLineNumber: position.lineNumber,
              endLineNumber: position.lineNumber,
              startColumn: position.column - reference.length,
              endColumn: word.endColumn,
            };
            return {
              suggestions: [
                ...symbols().map((item) => ({
                  label: formatExpression(item.expression),
                  detail: item.label,
                  insertText: formatExpression(item.expression),
                  kind: api.languages.CompletionItemKind.Variable,
                  range,
                })),
                ...FUNCTIONS.map((name) => ({
                  label: name,
                  insertText: name + "($0)",
                  insertTextRules:
                    api.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                  kind: api.languages.CompletionItemKind.Function,
                  range,
                })),
              ],
            };
          },
        }),
      ];
      return { dispose: () => disposables.forEach((item) => item?.dispose()) };
    },
  };
}
