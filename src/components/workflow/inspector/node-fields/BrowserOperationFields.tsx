import type { BrowserOperation } from '../../../../features/workflow';
import type { WorkflowNodeUpdater } from '../../../../features/workflow';
import { Input } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type BrowserOperationFieldsProps = Readonly<{
  operation: BrowserOperation;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 编辑打开网页节点的浏览器引用和网址表达式。 */
export function BrowserOperationFields({
  operation,
  onUpdate,
}: BrowserOperationFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="浏览器节点">
        <Input
          aria-label="浏览器节点"
          value={operation.browser.producer_node_id}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => updateOperation(onUpdate, {
            ...operation,
            browser: {
              ...operation.browser,
              producer_node_id: event.target.value,
            },
          })}
        />
      </InspectorField>
      <ValueExprFields
        value={operation.url}
        literalLabel="目标网址"
        expressionLocation={{ type: 'navigate_url' }}
        onChange={(url) => updateOperation(onUpdate, { ...operation, url })}
      />
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        请输入完整网址（以 http:// 或 https:// 开头）。请先打开浏览器。
      </p>
    </div>
  );
}

/** 只在判别联合仍是 Navigate 节点时写回操作。 */
function updateOperation(
  onUpdate: (updater: WorkflowNodeUpdater) => void,
  operation: BrowserOperation,
) {
  onUpdate((current) => current.kind === 'navigate'
    ? { ...current, operation, invalid: false }
    : current);
}
