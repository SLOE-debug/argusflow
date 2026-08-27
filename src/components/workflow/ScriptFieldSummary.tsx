import { StructuredFieldSummary } from './StructuredFieldSummary';
import {
  SCRIPT_RUNNER_LABELS,
  type ScriptRunner,
} from './commandScript';

type ScriptFieldSummaryProps = Readonly<{
  /** 当前脚本解释器。 */
  runner: ScriptRunner;
  /** 固定文本脚本源码。 */
  source: string;
  /** 请求在中央工作区编辑脚本。 */
  onEdit: () => void;
}>;

/** Inspector 中只展示脚本来源、行数与轻量预览。 */
export function ScriptFieldSummary({ runner, source, onEdit }: ScriptFieldSummaryProps) {
  const lineCount = source.length === 0 ? 0 : source.split('\n').length;
  return (
    <StructuredFieldSummary
      title="脚本"
      badge={SCRIPT_RUNNER_LABELS[runner]}
      status={`固定文本 · ${lineCount} 行`}
      preview={source}
      actionLabel="编辑脚本"
      onEdit={onEdit}
    />
  );
}
