import type * as Monaco from 'monaco-editor/editor/editor.api';
import { describe, expect, it, vi } from 'vitest';

import type { MonacoApi } from '../../../components/ui/monaco';
import {
  AQL_LANGUAGE_ID,
  registerAqlMonacoLanguage,
} from './MonacoAqlLanguage';
import type { AqlLanguageService } from './types';

describe('MonacoAqlLanguage', () => {
  it('maps the Rust/WASM document protocol to Monaco providers', () => {
    /** 注册过程中捕获的 provider，供行为断言直接调用。 */
    let semanticProvider: Monaco.languages.DocumentSemanticTokensProvider | null = null;
    let completionProvider: Monaco.languages.CompletionItemProvider | null = null;
    let hoverProvider: Monaco.languages.HoverProvider | null = null;
    let formattingProvider: Monaco.languages.DocumentFormattingEditProvider | null = null;
    let codeActionProvider: Monaco.languages.CodeActionProvider | null = null;
    const setModelMarkers = vi.fn();
    const source = 'b';
    const model = {
      getLanguageId: () => AQL_LANGUAGE_ID,
      getValue: () => source,
      getVersionId: () => 7,
      getFullModelRange: () => new TestRange(1, 1, 1, 2),
      onDidChangeContent: vi.fn(() => ({ dispose: vi.fn() })),
      uri: { scheme: 'inmemory', path: '/test.aql' },
    } as unknown as Monaco.editor.ITextModel;
    const monaco = {
      Range: TestRange,
      MarkerSeverity: { Error: 8, Warning: 4, Info: 2 },
      editor: {
        getModels: () => [model],
        onDidCreateModel: vi.fn(() => ({ dispose: vi.fn() })),
        onWillDisposeModel: vi.fn(() => ({ dispose: vi.fn() })),
        setModelMarkers,
      },
      languages: {
        CompletionItemKind: {
          Class: 5,
          Function: 1,
          Property: 9,
          Operator: 11,
          Value: 13,
        },
        CompletionItemInsertTextRule: { None: 0, InsertAsSnippet: 4 },
        register: vi.fn(),
        setLanguageConfiguration: vi.fn(),
        setMonarchTokensProvider: vi.fn(),
        registerDocumentSemanticTokensProvider: vi.fn((_language, provider) => {
          semanticProvider = provider;
          return { dispose: vi.fn() };
        }),
        registerCompletionItemProvider: vi.fn((_language, provider) => {
          completionProvider = provider;
          return { dispose: vi.fn() };
        }),
        registerHoverProvider: vi.fn((_language, provider) => {
          hoverProvider = provider;
          return { dispose: vi.fn() };
        }),
        registerDocumentFormattingEditProvider: vi.fn((_language, provider) => {
          formattingProvider = provider;
          return { dispose: vi.fn() };
        }),
        registerCodeActionProvider: vi.fn((_language, provider) => {
          codeActionProvider = provider;
          return { dispose: vi.fn() };
        }),
      },
    } as unknown as MonacoApi;
    const service: AqlLanguageService = {
      inspect: vi.fn(() => ({
        parsed: {
          diagnostics: [{
            code: 'unknown_role',
            severity: 'error',
            range: {
              start: { line: 0, utf16_column: 0 },
              end: { line: 0, utf16_column: 1 },
            },
            backend: null,
            params: { type: 'token', token: 'b' },
          }],
          semantic_tokens: [{
            kind: 'role',
            range: {
              start: { line: 0, utf16_column: 0 },
              end: { line: 0, utf16_column: 1 },
            },
          }],
          hir: null,
        },
        formatted_source: 'button()',
        canonical_source: null,
      })),
      completions: vi.fn(() => [{
        label: 'button',
        replacement_range: {
          start: { line: 0, utf16_column: 0 },
          end: { line: 0, utf16_column: 1 },
        },
        insert_text: 'button()',
        kind: 'role',
        detail: null,
      }]),
      hover: vi.fn(() => ({
        range: {
          start: { line: 0, utf16_column: 0 },
          end: { line: 0, utf16_column: 1 },
        },
        symbol: 'button',
        description_code: 'aql.hover.role',
      })),
      bracketPair: vi.fn(() => null),
      codeActions: vi.fn(() => [{
        range: {
          start: { line: 0, utf16_column: 0 },
          end: { line: 0, utf16_column: 1 },
        },
        new_text: 'button()',
      }]),
    };

    registerAqlMonacoLanguage(monaco, null);
    expect(monaco.languages.register).toHaveBeenCalledWith(expect.objectContaining({
      id: AQL_LANGUAGE_ID,
    }));
    expect(semanticProvider).toBeNull();

    registerAqlMonacoLanguage(monaco, service);
    expect(setModelMarkers).toHaveBeenCalledWith(
      model,
      AQL_LANGUAGE_ID,
      [expect.objectContaining({ message: '未知的元素角色', severity: 8 })],
    );

    const completion = completionProvider!.provideCompletionItems(
      model,
      { lineNumber: 1, column: 2 } as Monaco.Position,
      {} as Monaco.languages.CompletionContext,
      {} as Monaco.CancellationToken,
    ) as Monaco.languages.CompletionList;
    expect(service.completions).toHaveBeenCalledWith(source, {
      line: 0,
      utf16_column: 1,
    });
    expect(completion.suggestions[0]).toEqual(expect.objectContaining({
      insertText: 'button($0)',
      insertTextRules: 4,
    }));

    const hover = hoverProvider!.provideHover(
      model,
      { lineNumber: 1, column: 1 } as Monaco.Position,
      {} as Monaco.CancellationToken,
    ) as Monaco.languages.Hover;
    expect(hover.contents).toContainEqual({ value: '元素语义角色' });

    const semanticTokens = semanticProvider!.provideDocumentSemanticTokens(
      model,
      null,
      {} as Monaco.CancellationToken,
    ) as Monaco.languages.SemanticTokens;
    expect([...semanticTokens.data]).toEqual([0, 0, 1, 0, 0]);

    const formattingEdits = formattingProvider!.provideDocumentFormattingEdits(
      model,
      { tabSize: 2, insertSpaces: true },
      {} as Monaco.CancellationToken,
    ) as Monaco.languages.TextEdit[];
    expect(formattingEdits).toEqual([expect.objectContaining({ text: 'button()' })]);

    const actions = codeActionProvider!.provideCodeActions(
      model,
      new TestRange(1, 1, 1, 2) as unknown as Monaco.Range,
      { markers: [], trigger: 1 },
      {} as Monaco.CancellationToken,
    ) as Monaco.languages.CodeActionList;
    expect(actions.actions).toContainEqual(expect.objectContaining({
      kind: 'quickfix.aql',
      isPreferred: true,
    }));
  });
});

/** 测试所需的最小 Monaco Range 实现。 */
class TestRange {
  constructor(
    /** 范围起始行。 */
    public readonly startLineNumber: number,
    /** 范围起始列。 */
    public readonly startColumn: number,
    /** 范围结束行。 */
    public readonly endLineNumber: number,
    /** 范围结束列。 */
    public readonly endColumn: number,
  ) {}
}
