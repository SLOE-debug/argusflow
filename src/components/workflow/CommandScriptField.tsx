import type * as Monaco from 'monaco-editor/editor/editor.api';

import type {
  CommandRunner,
  ValueExpr,
} from '../../features/workflow/contracts';
import {
  InspectorEditorSection,
  type InspectorEditorLayout,
} from '../ui';
import { MonacoEditor } from '../ui/monaco';
import {
  ValueExprFields,
  type LiteralPresentation,
} from './ValueExprFields';

/** 具有脚本语义的 shell runner。 */
type ScriptRunner = Exclude<CommandRunner, 'direct'>;

type CommandScriptFieldProps = Readonly<{
  /** 当前脚本由哪一种 Windows shell 解释。 */
  runner: ScriptRunner;
  /** 当前命令节点的稳定标识。 */
  nodeId: string;
  /** 脚本文本或动态引用来源。 */
  value: ValueExpr;
  /** 写回完整 ValueExpr，不改变 Runtime 契约。 */
  onChange: (value: ValueExpr) => void;
}>;

/** runner 的稳定产品标签。 */
const SCRIPT_RUNNER_LABELS: Readonly<Record<ScriptRunner, string>> = {
  power_shell: 'PowerShell',
  cmd: 'CMD',
};

/** runner 对应的 Monaco 内置语言标识。 */
const SCRIPT_LANGUAGE_IDS: Readonly<Record<ScriptRunner, string>> = {
  power_shell: 'powershell',
  cmd: 'bat',
};

/** Shell 编辑器的稳定行为选项。 */
const SCRIPT_EDITOR_OPTIONS = {
  folding: true,
  glyphMargin: false,
  hover: { enabled: 'on', delay: 300, sticky: true, hidingDelay: 300 },
  lineNumbers: 'on',
  lineNumbersMinChars: 3,
  wordWrap: 'off',
} as const satisfies Monaco.editor.IStandaloneEditorConstructionOptions;

/** 为 PowerShell 与 CMD literal 提供 Monaco 多行编辑体验。 */
export function CommandScriptField({
  runner,
  nodeId,
  value,
  onChange,
}: CommandScriptFieldProps) {
  return (
    <InspectorEditorSection
      title="脚本"
      badge={SCRIPT_RUNNER_LABELS[runner]}
      expandable={value.type === 'literal'}
      renderContent={(layout) => (
        <ValueExprFields
          value={value}
          literalLabel="脚本内容"
          literalPresentation={createScriptPresentation(layout, runner, nodeId)}
          onChange={onChange}
        />
      )}
    />
  );
}

/** 以自定义字面量 renderer 接入 Monaco，同时保留 ValueExpr 来源切换。 */
function createScriptPresentation(
  layout: InspectorEditorLayout,
  runner: ScriptRunner,
  nodeId: string,
): LiteralPresentation {
  return {
    type: 'custom',
    render: ({ label, value, onChange }) => (
      <label className="flex flex-col gap-1.5 text-[10px] font-medium text-slate-500">
        {label}
        <MonacoEditor
          ariaLabel={label}
          value={value}
          language={SCRIPT_LANGUAGE_IDS[runner]}
          modelUri={`inmemory://argusflow/workflow/${encodeURIComponent(nodeId)}/command-script`}
          className={layout === 'expanded' ? 'h-[calc(100vh-170px)] min-h-[480px]' : 'h-[220px]'}
          options={SCRIPT_EDITOR_OPTIONS}
          onChange={onChange}
        />
      </label>
    ),
  };
}
