import type { BrowserOperation } from '../../features/workflow/contracts';
import type { WorkflowNodeUpdater } from '../../features/workflow/workflowModel';
import { Input } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type BrowserOperationFieldsProps = Readonly<{
  operation: BrowserOperation;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 编辑 Navigate 的 BrowserSession 引用和运行时 URL 表达式。 */
export function BrowserOperationFields({
  operation,
  onUpdate,
}: BrowserOperationFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="浏览器节点 ID">
        <Input
          aria-label="浏览器资源生产节点 ID"
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
        onChange={(url) => updateOperation(onUpdate, { ...operation, url })}
      />
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        URL 必须在运行时解析为绝对 HTTP(S) 地址；会话资源必须支配本节点。
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
