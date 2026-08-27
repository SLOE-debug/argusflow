import type * as Monaco from 'monaco-editor/editor/editor.api';

import type { CommandRunner } from '../../../../features/workflow';

/** 具有脚本文档语义的命令运行器。 */
export type ScriptRunner = Exclude<CommandRunner, 'direct'>;

/** Shell runner 的稳定产品标签。 */
export const SCRIPT_RUNNER_LABELS: Readonly<Record<ScriptRunner, string>> = {
  power_shell: 'PowerShell',
  cmd: 'CMD',
};

/** Shell runner 对应的 Monaco 语言标识。 */
export const SCRIPT_LANGUAGE_IDS: Readonly<Record<ScriptRunner, string>> = {
  power_shell: 'powershell',
  cmd: 'bat',
};

/** Workspace 中 Shell 编辑器共用的行为选项。 */
export const SCRIPT_EDITOR_OPTIONS = {
  folding: true,
  glyphMargin: false,
  hover: { enabled: 'on', delay: 300, sticky: true, hidingDelay: 300 },
  lineNumbers: 'on',
  lineNumbersMinChars: 3,
  wordWrap: 'off',
} as const satisfies Monaco.editor.IStandaloneEditorConstructionOptions;
