import { shikiToMonaco } from '@shikijs/monaco';
import { createHighlighterCore } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import batchLanguage from 'shiki/langs/bat.mjs';
import powerShellLanguage from 'shiki/langs/powershell.mjs';
import lightPlusTheme from 'shiki/themes/light-plus.mjs';

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
  /** @shikijs/monaco 依赖 monaco-editor-core，而应用使用带完整语言包的 monaco-editor；两者运行时 API 同源。 */
  shikiToMonaco(highlighter, monaco as Parameters<typeof shikiToMonaco>[1]);
}
