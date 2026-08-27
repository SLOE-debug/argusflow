import type * as Monaco from 'monaco-editor/editor/editor.api';

import type { MonacoApi } from '../../../components/ui/monaco';
import type {
  AqlDiagnostic,
  AqlDiagnosticSeverity,
  EditorPosition,
  EditorRange,
} from '../../workflow/contracts';
import { diagnosticLabel, hoverLabel } from './messages';
import type {
  AqlLanguageService,
  CompletionItem,
  CompletionItemKind,
  SyntaxToken,
  SyntaxTokenKind,
  TextEdit,
} from './types';

/** Monaco 中稳定的 AQL 语言标识。 */
export const AQL_LANGUAGE_ID = 'argusflow-aql';

/** Monaco semantic token 使用的 Light+ TextMate scope；顺序是 provider 协议的一部分。 */
const SEMANTIC_TOKEN_TYPES = [
  'entity.name.type',
  'entity.name.function',
  'variable.other.property',
  'entity.name.namespace',
  'keyword.operator',
  'string',
  'string.regexp',
  'constant.language',
  'constant.numeric',
  'punctuation',
] as const;

/** semantic token 类别到协议索引的强类型映射。 */
const SEMANTIC_TOKEN_INDEX = {
  role: 0,
  function: 1,
  property: 2,
  namespace: 3,
  operator: 4,
  string: 5,
  regex: 6,
  boolean: 7,
  integer: 8,
  punctuation: 9,
} as const satisfies Readonly<Record<HighlightableSyntaxTokenKind, number>>;

/** 需要覆盖 Monarch 基础着色的 Rust semantic token 类别。 */
type HighlightableSyntaxTokenKind = Exclude<SyntaxTokenKind, 'trivia' | 'unknown'>;

/** WASM 初始化前供 Monarch 使用的 AQL 函数清单。 */
const AQL_FALLBACK_FUNCTIONS = ['any', 'not', 'first', 'nth', 'css'] as const;

/** WASM 初始化前供 Monarch 使用的 AQL 属性清单。 */
const AQL_FALLBACK_PROPERTIES = [
  'name',
  'key',
  'value',
  'enabled',
  'visible',
  'focused',
  'checked',
  'selected',
] as const;

/** WASM 初始化前供 Monarch 使用的单词运算符清单。 */
const AQL_FALLBACK_OPERATORS = [
  'contains',
  'starts_with',
  'ends_with',
  'matches',
] as const;

/** WASM 初始化前供 Monarch 使用的 AQL v1 角色清单。 */
const AQL_FALLBACK_ROLES = [
  'window',
  'dialog',
  'pane',
  'button',
  'textbox',
  'checkbox',
  'radio',
  'combobox',
  'list',
  'list_item',
  'tree',
  'tree_item',
  'tab',
  'tab_item',
  'menu',
  'menu_item',
  'link',
  'image',
  'table',
  'row',
  'cell',
  'document',
  'text',
] as const;

/** 当前 WASM 服务由 provider 闭包读取，避免重复注册全局语言。 */
let activeService: AqlLanguageService | null = null;

/** 当前页面生命周期内是否已经完成 AQL 语言定义注册。 */
let languageRegistered = false;

/** 当前页面生命周期内是否已经完成 AQL provider 注册。 */
let providersRegistered = false;

/** 已订阅诊断刷新的 AQL 文档及其变更监听。 */
const diagnosticSubscriptions = new Map<
  Monaco.editor.ITextModel,
  Monaco.IDisposable
>();

/** 注册或更新 Monaco 使用的 AQL WASM 语言服务。 */
export function registerAqlMonacoLanguage(
  monaco: MonacoApi,
  service: AqlLanguageService | null,
): void {
  if (!languageRegistered) {
    registerLanguageDefinition(monaco);
    languageRegistered = true;
  }
  if (!service) {
    return;
  }

  activeService = service;
  if (!providersRegistered) {
    registerLanguageProviders(monaco);
    monaco.editor.onDidCreateModel((model) => attachDiagnostics(monaco, model));
    monaco.editor.onWillDisposeModel((model) => {
      diagnosticSubscriptions.get(model)?.dispose();
      diagnosticSubscriptions.delete(model);
    });
    providersRegistered = true;
  }

  monaco.editor.getModels().forEach((model) => attachDiagnostics(monaco, model));
}

/** 注册括号、自动闭合和 WASM 就绪前即可生效的 Monarch 着色。 */
function registerLanguageDefinition(monaco: MonacoApi): void {
  monaco.languages.register({
    id: AQL_LANGUAGE_ID,
    aliases: ['AQL', 'ArgusFlow Query Language'],
    extensions: ['.aql'],
  });
  monaco.languages.setLanguageConfiguration(AQL_LANGUAGE_ID, {
    brackets: [['(', ')']],
    autoClosingPairs: [
      { open: '(', close: ')' },
      { open: '"', close: '"' },
      { open: '/', close: '/' },
    ],
    surroundingPairs: [
      { open: '(', close: ')' },
      { open: '"', close: '"' },
      { open: '/', close: '/' },
    ],
    wordPattern: /[A-Za-z_][\w-]*/,
  });
  monaco.languages.setMonarchTokensProvider(AQL_LANGUAGE_ID, {
    defaultToken: 'unknown',
    functions: AQL_FALLBACK_FUNCTIONS,
    properties: AQL_FALLBACK_PROPERTIES,
    wordOperators: AQL_FALLBACK_OPERATORS,
    roles: AQL_FALLBACK_ROLES,
    tokenizer: {
      root: [
        [/\s+/, 'trivia'],
        [/"(?:\\.|[^"\\])*"?/, 'string'],
        [/\/(?:\\.|[^/\\])+\/[a-z]*/, 'string.regexp'],
        [/\b(?:true|false)\b/, 'constant.language'],
        [/\b\d+\b/, 'constant.numeric'],
        [/(?:!=|~=|\^=|\$=|\*=|>=|<=|=|>|<)/, 'keyword.operator'],
        [/[(),]/, 'punctuation'],
        [/\b(?:uia|dom)\.[A-Za-z_][\w-]*/, 'entity.name.namespace'],
        [/[A-Za-z_][\w-]*/, {
          cases: {
            '@functions': 'entity.name.function',
            '@properties': 'variable.other.property',
            '@wordOperators': 'keyword.operator',
            '@roles': 'entity.name.type',
            '@default': 'source',
          },
        }],
      ],
    },
  });
}

/** 把 Rust/WASM 能力映射到 Monaco 标准 provider。 */
function registerLanguageProviders(monaco: MonacoApi): void {
  monaco.languages.registerDocumentSemanticTokensProvider(AQL_LANGUAGE_ID, {
    getLegend: () => ({ tokenTypes: [...SEMANTIC_TOKEN_TYPES], tokenModifiers: [] }),
    provideDocumentSemanticTokens: (model) => ({
      resultId: String(model.getVersionId()),
      data: encodeSemanticTokens(currentService().inspect(model.getValue()).parsed.semantic_tokens),
    }),
    releaseDocumentSemanticTokens: () => undefined,
  });

  monaco.languages.registerCompletionItemProvider(AQL_LANGUAGE_ID, {
    triggerCharacters: ['(', ',', '='],
    provideCompletionItems: (model, position) => ({
      suggestions: currentService()
        .completions(model.getValue(), toEditorPosition(position))
        .map((item) => toMonacoCompletion(monaco, item)),
    }),
  });

  monaco.languages.registerHoverProvider(AQL_LANGUAGE_ID, {
    provideHover: (model, position) => {
      const hover = currentService().hover(model.getValue(), toEditorPosition(position));
      if (!hover) {
        return null;
      }
      return {
        range: toMonacoRange(monaco, hover.range),
        contents: [
          { value: `**${escapeMarkdown(hover.symbol)}**` },
          { value: hoverLabel(hover.description_code) },
        ],
      };
    },
  });

  monaco.languages.registerDocumentFormattingEditProvider(AQL_LANGUAGE_ID, {
    displayName: 'AQL 格式化器',
    provideDocumentFormattingEdits: (model) => {
      const formattedSource = currentService().inspect(model.getValue()).formatted_source;
      return formattedSource && formattedSource !== model.getValue()
        ? [{ range: model.getFullModelRange(), text: formattedSource }]
        : [];
    },
  });

  monaco.languages.registerCodeActionProvider(AQL_LANGUAGE_ID, {
    provideCodeActions: (model, _range, context) => {
      const edits = currentService().codeActions(model.getValue());
      return {
        actions: edits.length === 0 ? [] : [{
          title: '修正查找条件',
          kind: 'quickfix.aql',
          isPreferred: true,
          diagnostics: context.markers,
          edit: {
            edits: edits.map((edit) => ({
              resource: model.uri,
              versionId: model.getVersionId(),
              textEdit: toMonacoTextEdit(monaco, edit),
            })),
          },
        }],
        dispose: () => undefined,
      };
    },
  }, { providedCodeActionKinds: ['quickfix.aql'] });
}

/** 为 AQL 模型安装同步诊断；其他语言模型不受影响。 */
function attachDiagnostics(
  monaco: MonacoApi,
  model: Monaco.editor.ITextModel,
): void {
  if (model.getLanguageId() !== AQL_LANGUAGE_ID || diagnosticSubscriptions.has(model)) {
    return;
  }
  const updateMarkers = () => {
    const diagnostics = currentService().inspect(model.getValue()).parsed.diagnostics;
    monaco.editor.setModelMarkers(
      model,
      AQL_LANGUAGE_ID,
      diagnostics.map((diagnostic) => toMonacoMarker(monaco, diagnostic)),
    );
  };
  diagnosticSubscriptions.set(model, model.onDidChangeContent(updateMarkers));
  updateMarkers();
}

/** 读取已经配置的服务；注册顺序保证 provider 调用前一定存在。 */
function currentService(): AqlLanguageService {
  if (!activeService) {
    throw new Error('AQL Monaco language service has not been configured.');
  }
  return activeService;
}

/** Monaco 使用一基行列，WASM 协议使用零基 UTF-16 行列。 */
function toEditorPosition(position: Monaco.Position): EditorPosition {
  return {
    line: position.lineNumber - 1,
    utf16_column: position.column - 1,
  };
}

/** 把半开 UTF-16 协议范围映射到 Monaco 一基范围。 */
function toMonacoRange(monaco: MonacoApi, range: EditorRange): Monaco.Range {
  return new monaco.Range(
    range.start.line + 1,
    range.start.utf16_column + 1,
    range.end.line + 1,
    range.end.utf16_column + 1,
  );
}

/** 将补全类别和 replacement range 转为 Monaco suggestion。 */
function toMonacoCompletion(
  monaco: MonacoApi,
  item: CompletionItem,
): Monaco.languages.CompletionItem {
  const insertsPair = item.insert_text.endsWith('()');
  return {
    label: item.label,
    kind: completionKind(monaco, item.kind),
    detail: item.detail ?? undefined,
    range: toMonacoRange(monaco, item.replacement_range),
    insertText: insertsPair
      ? `${item.insert_text.slice(0, -1)}$0)`
      : item.insert_text,
    insertTextRules: insertsPair
      ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
      : monaco.languages.CompletionItemInsertTextRule.None,
  };
}

/** AQL 补全类别到 Monaco 图标类别的穷尽映射。 */
function completionKind(
  monaco: MonacoApi,
  kind: CompletionItemKind,
): Monaco.languages.CompletionItemKind {
  switch (kind) {
    case 'role':
      return monaco.languages.CompletionItemKind.Class;
    case 'function':
      return monaco.languages.CompletionItemKind.Function;
    case 'property':
      return monaco.languages.CompletionItemKind.Property;
    case 'operator':
      return monaco.languages.CompletionItemKind.Operator;
    case 'value':
      return monaco.languages.CompletionItemKind.Value;
  }
}

/** 把排序后的单行 Rust token 编码为 Monaco delta token 流。 */
function encodeSemanticTokens(tokens: readonly SyntaxToken[]): Uint32Array {
  const orderedTokens = [...tokens]
    .filter(isHighlightableToken)
    .filter((token) => token.range.start.line === token.range.end.line)
    .filter((token) => token.range.end.utf16_column > token.range.start.utf16_column)
    .sort((left, right) => left.range.start.line - right.range.start.line
      || left.range.start.utf16_column - right.range.start.utf16_column);
  const data: number[] = [];
  let previousLine = 0;
  let previousColumn = 0;
  orderedTokens.forEach((token) => {
    const line = token.range.start.line;
    const column = token.range.start.utf16_column;
    data.push(
      line - previousLine,
      line === previousLine ? column - previousColumn : column,
      token.range.end.utf16_column - column,
      SEMANTIC_TOKEN_INDEX[token.kind],
      0,
    );
    previousLine = line;
    previousColumn = column;
  });
  return Uint32Array.from(data);
}

/** 空白与未知标识符沿用 Monarch 结果，避免 semantic token 覆盖基础主题。 */
function isHighlightableToken(
  token: SyntaxToken,
): token is SyntaxToken & { readonly kind: HighlightableSyntaxTokenKind } {
  return token.kind !== 'trivia' && token.kind !== 'unknown';
}

/** 将 Rust code action 编辑映射到 Monaco workspace edit。 */
function toMonacoTextEdit(
  monaco: MonacoApi,
  edit: TextEdit,
): Monaco.languages.TextEdit {
  return { range: toMonacoRange(monaco, edit.range), text: edit.new_text };
}

/** 将结构化诊断映射为 Monaco marker。 */
function toMonacoMarker(
  monaco: MonacoApi,
  diagnostic: AqlDiagnostic,
): Monaco.editor.IMarkerData {
  const range = diagnostic.range;
  return {
    code: diagnostic.code,
    severity: markerSeverity(monaco, diagnostic.severity),
    message: diagnosticLabel(diagnostic),
    source: 'AQL',
    startLineNumber: (range?.start.line ?? 0) + 1,
    startColumn: (range?.start.utf16_column ?? 0) + 1,
    endLineNumber: (range?.end.line ?? 0) + 1,
    endColumn: (range?.end.utf16_column ?? 1) + 1,
  };
}

/** AQL 诊断严重级别到 Monaco marker 严重级别的穷尽映射。 */
function markerSeverity(
  monaco: MonacoApi,
  severity: AqlDiagnosticSeverity,
): Monaco.MarkerSeverity {
  switch (severity) {
    case 'error':
      return monaco.MarkerSeverity.Error;
    case 'warning':
      return monaco.MarkerSeverity.Warning;
    case 'information':
      return monaco.MarkerSeverity.Info;
  }
}

/** 避免 token 文本改变 Monaco Hover Markdown 结构。 */
function escapeMarkdown(value: string): string {
  return value.replace(/[\\`*_{}[\]()#+\-.!]/g, '\\$&');
}
