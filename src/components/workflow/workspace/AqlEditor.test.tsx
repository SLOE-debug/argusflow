import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useLanguageDocument } from '../../../features/aql-editor/language/useLanguageDocument';
import { AqlEditor } from '../../../features/aql-editor/view/AqlEditor';
import type { AqlQuery } from '../../../features/workflow/model/contracts';

vi.mock('../../../features/aql-editor/language/useLanguageDocument', () => ({
  useLanguageDocument: vi.fn(),
}));

describe('AqlEditor', () => {
  beforeEach(() => {
    const languageDocument = {
      parsed: { diagnostics: [], semantic_tokens: [], hir: {} },
      formatted_source: 'button(\n    name = "保存",\n    enabled = true\n)',
      canonical_source: 'button(enabled=true,name="保存")',
    } as const;
    vi.mocked(useLanguageDocument).mockReturnValue({
      phase: 'ready',
      message: null,
      document: languageDocument,
      service: {
        inspect: vi.fn(() => languageDocument),
        completions: vi.fn(() => []),
        hover: vi.fn(() => null),
        codeActions: vi.fn(() => []),
      },
    });
  });

  it('shows only language tools and exposes the standard format command', () => {
    const onChange = vi.fn();
    const query = { language_version: 3 as const, bindings: {}, source: 'button(name="保存",enabled=true)' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-format"
        onChange={onChange}
      />,
    );

    expect(screen.queryByText(/执行方式/)).not.toBeInTheDocument();
    expect(screen.queryByText('查找条件可以使用')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '格式化' }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it('does not render a success banner when the document is already formatted', () => {
    const query = { language_version: 3 as const, bindings: {}, source: 'button()' };
    const languageDocument = {
      parsed: { diagnostics: [], semantic_tokens: [], hir: {} },
      formatted_source: query.source,
      canonical_source: query.source,
    } as const;
    vi.mocked(useLanguageDocument).mockReturnValue({
      phase: 'ready',
      message: null,
      document: languageDocument,
      service: {
        inspect: vi.fn(() => languageDocument),
        completions: vi.fn(() => []),
        hover: vi.fn(() => null),
        codeActions: vi.fn(() => []),
      },
    });

    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-clean"
        onChange={vi.fn()}
      />,
    );

    expect(screen.queryByText('已格式化')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '格式化' })).not.toBeInTheDocument();
  });

  it('shows syntax diagnostics returned by the language service', () => {
    const query = { language_version: 3 as const, bindings: {}, source: 'button(' };
    const languageDocument = {
      parsed: {
        diagnostics: [{
          code: 'missing_right_parenthesis' as const,
          severity: 'error' as const,
          range: {
            start: { line: 0, utf16_column: 6 },
            end: { line: 0, utf16_column: 7 },
          },
          backend: null,
          params: { type: 'none' as const },
        }],
        semantic_tokens: [],
        hir: {},
      },
      formatted_source: null,
      canonical_source: null,
    };
    vi.mocked(useLanguageDocument).mockReturnValue({
      phase: 'ready',
      message: null,
      document: languageDocument,
      service: {
        inspect: vi.fn(() => languageDocument),
        completions: vi.fn(() => []),
        hover: vi.fn(() => null),
        codeActions: vi.fn(() => []),
      },
    });

    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-diagnostic"
        onChange={vi.fn()}
      />,
    );

    expect(screen.getByRole('alert')).toHaveTextContent('第 1 行，第 7 列：缺少右括号');
    expect(screen.getByRole('button', { name: '请先修复语法错误' })).toBeDisabled();
  });

  it('writes Monaco document changes back to the versioned query', () => {
    const onChange = vi.fn();
    const query = { language_version: 3 as const, bindings: {}, source: 'button' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-edit"
        onChange={onChange}
      />,
    );
    const input = screen.getByRole('textbox', { name: 'AQL 查找条件' });
    expect(input).toHaveAttribute('data-language', 'argusflow-aql');
    fireEvent.change(input, { target: { value: 'button()' } });

    expect(onChange).toHaveBeenCalledWith({ language_version: 3 as const, bindings: {}, source: 'button()' });
  });

  it('does not expose a separate token-help button', () => {
    const query = { language_version: 3 as const, bindings: {}, source: 'button' };
    const { rerender } = render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-hover"
        onChange={vi.fn()}
      />,
    );
    expect(screen.queryByRole('button', { name: '说明' })).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'AQL 查找条件' }), {
      target: { value: 'button(name = "保存")' },
    });
    rerender(
      <AqlEditor
        query={{ ...query, source: 'button(name = "保存")' }}
        modelUri="inmemory://test/aql-hover"
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByRole('textbox', { name: 'AQL 查找条件' })).toHaveValue(
      'button(name = "保存")',
    );
  });

  it('renders as a Workspace editor without a modal dialog', () => {
    const query = { language_version: 3 as const, bindings: {}, source: 'button' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-expand"
        onChange={vi.fn()}
      />,
    );

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'AQL 查找条件' })).toHaveValue('button');
  });

  it('keeps native composition input as the document source', () => {
    function CompositionHarness() {
      const [query, setQuery] = useState<AqlQuery>({
        language_version: 3,
        source: 'button(name = "")',
        bindings: {},
      });
      return (
        <AqlEditor
          query={query}
          modelUri="inmemory://test/aql-composition"
          onChange={setQuery}
        />
      );
    }

    render(<CompositionHarness />);
    const input = screen.getByRole('textbox', { name: 'AQL 查找条件' });
    fireEvent.compositionStart(input);
    fireEvent.change(input, { target: { value: 'button(name = "保存")' } });
    fireEvent.compositionEnd(input);

    expect(input).toHaveValue('button(name = "保存")');
  });
});
