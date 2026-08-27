import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import type * as Monaco from 'monaco-editor/editor/editor.api';

import { registerShellSyntaxHighlighting } from './shellSyntaxHighlighting';

/** 应用内按需加载的 Monaco 公共 API。 */
export type MonacoApi = typeof Monaco;

/** 复用编辑器模块初始化，避免多个 Inspector 重复加载 Monaco。 */
let monacoPromise: Promise<MonacoApi> | null = null;

/**
 * 加载 Monaco 编辑器内核与 Windows shell 语言定义。
 *
 * Worker 通过 Vite 的模块 Worker 管线生成独立资源，适用于 Tauri 的同源 CSP。
 */
export function loadMonacoEditor(): Promise<MonacoApi> {
  monacoPromise ??= initializeMonaco();
  return monacoPromise;
}

/** 注册 Worker、基础语言与 ArgusFlow 浅色主题。 */
async function initializeMonaco(): Promise<MonacoApi> {
  globalThis.MonacoEnvironment = {
    getWorker: () => new EditorWorker(),
  };

  const monaco = await import('monaco-editor/editor/editor.api');
  await Promise.all([
    import('monaco-editor/features/register.all'),
    import('monaco-editor/languages/definitions/powershell/register'),
    import('monaco-editor/languages/definitions/bat/register'),
  ]);

  await registerShellSyntaxHighlighting(monaco);

  return monaco;
}
