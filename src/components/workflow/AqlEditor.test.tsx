import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useLanguageDocument } from '../../features/aql-editor/language/useLanguageDocument';
import { useAqlInspection } from '../../features/workflow/useAqlInspection';
import { AqlEditor } from '../../features/aql-editor/view/AqlEditor';

vi.mock('../../features/workflow/useAqlInspection', () => ({
  useAqlInspection: vi.fn(),
}));

vi.mock('../../features/aql-editor/language/useLanguageDocument', () => ({
  useLanguageDocument: vi.fn(),
}));

describe('AqlEditor', () => {
  beforeEach(() => {
    const languageDocument = {
      parsed: { diagnostics: [], semantic_tokens: [], hir: {} },
      formatted_source: 'button(\n    name = "保存",\n    enabled = true\n)',
      canonical_source: 'button(enabled=true,name="保存")',
    } as const;
    vi.mocked(useAqlInspection).mockReturnValue({
      phase: 'ready',
      message: null,
      inspection: {
        status: 'valid',
        canonical_source: 'button(enabled=true,name="保存")',
        portability: { type: 'portable' },
        diagnostics: [],
        planning: {
          selected_backend: 'windows_uia',
          candidates: [{
            backend: 'windows_uia',
            branch_path: [],
            support: 'native',
            cost: 'low',
            availability: 'ready',
            context_fitness: 'good',
            portability: { type: 'portable' },
            steps: [{ kind: 'pushdown', summary: '2 native conditions' }],
            diagnostics: [],
          }],
        },
      },
    });
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

  it('shows planner selection and exposes the standard format command', () => {
    const onChange = vi.fn();
    const query = { language_version: 1 as const, source: 'button(name="保存",enabled=true)' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-format"
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: {
            allow: ['windows_uia'],
            deny: [],
            prefer: ['windows_uia'],
          },
        }}
        onChange={onChange}
      />,
    );

    expect(screen.getByText('执行方式：Windows UI 自动化')).toBeVisible();
    expect(screen.getByText('查找条件可以使用')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '格式化' }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it('explains that a document already matches the formatter output', () => {
    const query = { language_version: 1 as const, source: 'button()' };
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
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
        onChange={vi.fn()}
      />,
    );

    expect(screen.getByText('已格式化')).toBeVisible();
    expect(screen.queryByRole('button', { name: '格式化' })).not.toBeInTheDocument();
  });

  it('writes Monaco document changes back to the versioned query', () => {
    const onChange = vi.fn();
    const query = { language_version: 1 as const, source: 'button' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-edit"
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
        onChange={onChange}
      />,
    );
    const input = screen.getByRole('textbox', { name: 'AQL 查找条件' });
    expect(input).toHaveAttribute('data-language', 'argusflow-aql');
    fireEvent.change(input, { target: { value: 'button()' } });

    expect(onChange).toHaveBeenCalledWith({ language_version: 1, source: 'button()' });
  });

  it('does not expose a separate token-help button', () => {
    const query = { language_version: 1 as const, source: 'button' };
    const { rerender } = render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-hover"
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
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
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query: { ...query, source: 'button(name = "保存")' } },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByRole('textbox', { name: 'AQL 查找条件' })).toHaveValue(
      'button(name = "保存")',
    );
  });

  it('renders as a Workspace editor without a modal dialog', () => {
    const query = { language_version: 1 as const, source: 'button' };
    render(
      <AqlEditor
        query={query}
        modelUri="inmemory://test/aql-expand"
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
        onChange={vi.fn()}
      />,
    );

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'AQL 查找条件' })).toHaveValue('button');
  });

  it('keeps native composition input as the document source', () => {
    function CompositionHarness() {
      const [query, setQuery] = useState({
        language_version: 1 as const,
        source: 'button(name = "")',
      });
      return (
        <AqlEditor
          query={query}
          modelUri="inmemory://test/aql-composition"
          target={{
            scope: { type: 'current' },
            locator: { type: 'query', query },
            backend_policy: { allow: [], deny: [], prefer: [] },
          }}
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
