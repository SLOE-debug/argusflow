import type {
  ValueExpr,
} from '../../features/workflow/contracts';
import {
  ValueExprFields,
  type LiteralPresentation,
} from './ValueExprFields';
import { ScriptFieldSummary } from './ScriptFieldSummary';
import type { ScriptRunner } from './commandScript';

type CommandScriptFieldProps = Readonly<{
  /** 当前脚本由哪一种 Windows shell 解释。 */
  runner: ScriptRunner;
  /** 脚本文本或动态引用来源。 */
  value: ValueExpr;
  /** 写回完整 ValueExpr，不改变 Runtime 契约。 */
  onChange: (value: ValueExpr) => void;
  /** 请求 Workspace 打开当前节点的脚本文档。 */
  onOpenEditor: () => void;
}>;

/** 在 Inspector 保留 ValueExpr 来源配置，并用摘要替代 literal Monaco。 */
export function CommandScriptField({
  runner,
  value,
  onChange,
  onOpenEditor,
}: CommandScriptFieldProps) {
  return (
    <ValueExprFields
      value={value}
      literalLabel="脚本内容"
      literalPresentation={createScriptPresentation(runner, onOpenEditor)}
      onChange={onChange}
    />
  );
}

/** 以自定义字面量摘要保留 ValueExpr 的强类型来源切换。 */
function createScriptPresentation(
  runner: ScriptRunner,
  onOpenEditor: () => void,
): LiteralPresentation {
  return {
    type: 'custom',
    render: ({ value }) => (
      <ScriptFieldSummary
        runner={runner}
        source={value}
        onEdit={onOpenEditor}
      />
    ),
  };
}
