import { shikiToMonaco } from '@shikijs/monaco';
import { codeToTokensBase } from 'shiki/core';
import { describe, expect, it, vi } from 'vitest';

import type { MonacoApi } from './monacoLoader';
import {
  MONACO_EDITOR_THEME,
  registerShellSyntaxHighlighting,
} from './shellSyntaxHighlighting';

vi.mock('@shikijs/monaco', () => ({ shikiToMonaco: vi.fn() }));

describe('Shell syntax highlighting', () => {
  it('registers the third-party PowerShell and CMD grammars with Light+', async () => {
    const monaco = {} as MonacoApi;

    await registerShellSyntaxHighlighting(monaco);

    const highlighter = vi.mocked(shikiToMonaco).mock.calls[0]?.[0];
    expect(MONACO_EDITOR_THEME).toBe('light-plus');
    expect(highlighter?.getLoadedLanguages()).toEqual(expect.arrayContaining(['powershell', 'bat']));
    expect(highlighter?.getLoadedThemes()).toEqual(['light-plus']);
    expect(shikiToMonaco).toHaveBeenCalledWith(highlighter, monaco);

    const powerShellTokens = highlighter
      ? codeToTokensBase(
        highlighter,
        '[Console]::WriteLine($value)',
        { lang: 'powershell', theme: MONACO_EDITOR_THEME },
      )
      : [];
    const commandTokens = highlighter
      ? codeToTokensBase(
        highlighter,
        '@echo off\r\nset value=42',
        { lang: 'bat', theme: MONACO_EDITOR_THEME },
      )
      : [];
    expect(uniqueTokenColors(powerShellTokens).size).toBeGreaterThan(1);
    expect(uniqueTokenColors(commandTokens).size).toBeGreaterThan(1);
  });
});

/** 汇总 TextMate Grammar 经 Light+ 主题解析后的可见 token 颜色。 */
function uniqueTokenColors(
  lines: readonly (readonly Readonly<{ color?: string }>[])[],
): ReadonlySet<string> {
  return new Set(lines.flatMap((line) => line.map((token) => token.color ?? '')));
}
