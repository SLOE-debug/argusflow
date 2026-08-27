import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import type * as Monaco from 'monaco-editor/editor/editor.api';

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

  monaco.editor.defineTheme('argusflow-light', {
    base: 'vs',
    inherit: true,
    colors: {
      'editor.background': '#ffffff',
      'editor.lineHighlightBackground': '#f8fafc',
      'editorGutter.background': '#ffffff',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#cbd5e1',
    },
    rules: [
      { token: 'role', foreground: '1D4ED8', fontStyle: 'bold' },
      { token: 'function', foreground: '7C3AED' },
      { token: 'property', foreground: '0369A1' },
      { token: 'namespace', foreground: '0F766E' },
      { token: 'operator', foreground: 'BE123C' },
      { token: 'string', foreground: '15803D' },
      { token: 'regex', foreground: 'B45309' },
      { token: 'boolean', foreground: '6D28D9' },
      { token: 'integer', foreground: 'B45309' },
      { token: 'punctuation', foreground: '475569' },
    ],
  });

  return monaco;
}
