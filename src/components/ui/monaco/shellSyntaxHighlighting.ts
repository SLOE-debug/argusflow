import { shikiToMonaco } from '@shikijs/monaco';
import { createHighlighterCore } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import batchLanguage from 'shiki/langs/bat';
import powerShellLanguage from 'shiki/langs/powershell';
import lightPlusTheme from 'shiki/themes/light-plus';

import type { MonacoApi } from './monacoLoader';

/** Shell 与 AQL 编辑器共同使用的 VS Code Light+ 主题标识。 */
export const MONACO_EDITOR_THEME = 'light-plus';

/**
 * 使用 Shiki 的 VS Code TextMate Grammar 替换 Monaco 内置 Shell tokenizer。
 *
 * 仅装载 PowerShell、Batch/CMD 和 Light+，并使用 JavaScript RegExp 引擎避免引入额外 WASM。
 */
export async function registerShellSyntaxHighlighting(monaco: MonacoApi): Promise<void> {
  const highlighter = await createHighlighterCore({
    engine: createJavaScriptRegexEngine(),
    langs: [powerShellLanguage, batchLanguage],
    themes: [lightPlusTheme],
  });
  shikiToMonaco(highlighter, monaco);
}
