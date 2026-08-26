import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useLanguageDocument } from '../../features/aql-editor/language/useLanguageDocument';
import { useAqlInspection } from '../../features/workflow/useAqlInspection';
import { AqlEditor } from './AqlEditor';

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
        bracketPair: vi.fn(() => null),
        codeActions: vi.fn(() => []),
      },
    });
  });

  it('shows planner selection and formats without reordering predicates', () => {
    const onChange = vi.fn();
    const query = { language_version: 1 as const, source: 'button(name="保存",enabled=true)' };
    render(
      <AqlEditor
        query={query}
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

    expect(screen.getByText('自动选择：Windows UI')).toBeVisible();
    expect(screen.getByText('查询可用')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '格式化' }));
    expect(onChange).toHaveBeenCalledWith({
      language_version: 1,
      source: 'button(\n    name = "保存",\n    enabled = true\n)',
    });
  });

  it('pairs brackets while keeping textarea as the input model', () => {
    const onChange = vi.fn();
    const query = { language_version: 1 as const, source: 'button' };
    render(
      <AqlEditor
        query={query}
        target={{
          scope: { type: 'current' },
          locator: { type: 'query', query },
          backend_policy: { allow: [], deny: [], prefer: [] },
        }}
        onChange={onChange}
      />,
    );
    const input = screen.getByRole('textbox', { name: 'AQL 查询' }) as HTMLTextAreaElement;
    input.setSelectionRange(6, 6);
    fireEvent.select(input);
    fireEvent.keyDown(input, { key: '(' });

    expect(onChange).toHaveBeenCalledWith({ language_version: 1, source: 'button()' });
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
    const input = screen.getByRole('textbox', { name: 'AQL 查询' });
    fireEvent.compositionStart(input);
    fireEvent.change(input, { target: { value: 'button(name = "保存")' } });
    fireEvent.compositionEnd(input);

    expect(input).toHaveValue('button(name = "保存")');
  });
});
