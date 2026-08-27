import { MonacoEditor } from '../../../ui/monaco';
import {
  SCRIPT_EDITOR_OPTIONS,
  SCRIPT_LANGUAGE_IDS,
  type ScriptRunner,
} from './commandScript';

type CommandScriptEditorProps = Readonly<{
  /** 当前脚本由哪一种 shell 解释。 */
  runner: ScriptRunner;
  /** 当前命令节点的稳定标识。 */
  nodeId: string;
  /** 固定文本脚本源码。 */
  source: string;
  /** 实时写回所属节点文档。 */
  onChange: (source: string) => void;
}>;

/** 只在 Workspace 中挂载的 PowerShell/CMD Monaco 编辑器。 */
export function CommandScriptEditor({
  runner,
  nodeId,
  source,
  onChange,
}: CommandScriptEditorProps) {
  return (
    <div className="h-full min-h-0 bg-white p-2">
      <MonacoEditor
        ariaLabel="脚本内容"
        value={source}
        language={SCRIPT_LANGUAGE_IDS[runner]}
        modelUri={`inmemory://argusflow/workflow/${encodeURIComponent(nodeId)}/command-script`}
        className="h-full min-h-0"
        options={SCRIPT_EDITOR_OPTIONS}
        onChange={onChange}
      />
    </div>
  );
}
