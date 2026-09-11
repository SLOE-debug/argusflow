import type * as Monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import { activeTheme, subscribeTheme } from '../../../features/themes';
/** Monaco 与工作台共用语义色；新增主题无须修改编辑器组件。 */
export function bindEditorTheme(monaco: typeof Monaco): () => void {
  const apply = () => {
    const theme = activeTheme();
    monaco.editor.defineTheme('argusflow', {
      base: theme.scheme === 'dark' ? 'vs-dark' : 'vs', inherit: true,
      rules: [
        { token: 'keyword', foreground: theme.syntax.keyword },
        { token: 'string', foreground: theme.syntax.string },
        { token: 'number', foreground: theme.syntax.number },
        { token: 'comment', foreground: theme.syntax.comment },
      ],
      colors: {
        'editor.background': theme.colors.surface, 'editor.foreground': theme.colors.text,
        'editorLineNumber.foreground': theme.colors.muted, 'editor.selectionBackground': theme.colors.accentSoft,
        'editorWidget.background': theme.colors.panel, 'editorWidget.border': theme.colors.border,
        'editorSuggestWidget.background': theme.colors.panel, 'editorSuggestWidget.foreground': theme.colors.text,
      },
    });
    monaco.editor.setTheme('argusflow');
  };
  apply();
  return subscribeTheme(apply);
}
