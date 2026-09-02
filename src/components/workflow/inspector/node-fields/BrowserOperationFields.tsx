import type {
  BrowserOperation,
  WorkflowResourceCatalog,
} from '../../../../features/workflow';
import type { WorkflowNodeUpdater } from '../../../../features/workflow';
import { ResourceNodeField } from '../common/ResourceNodeField';
import { INSPECTOR_HELP_CLASS_NAME } from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type BrowserOperationFieldsProps = Readonly<{
  operation: BrowserOperation;
  resourceCatalog: WorkflowResourceCatalog;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 编辑打开网页节点的浏览器引用和网址表达式。 */
export function BrowserOperationFields({
  operation,
  resourceCatalog,
  onUpdate,
}: BrowserOperationFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <ResourceNodeField
        kind="browser"
        resource={operation.browser}
        catalog={resourceCatalog}
        onChange={(browser) => updateOperation(onUpdate, { ...operation, browser })}
      />
      <ValueExprFields
        value={operation.url}
        literalLabel="目标网址"
        expressionLocation={{ type: 'navigate_url' }}
        onChange={(url) => updateOperation(onUpdate, { ...operation, url })}
      />
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        请输入以 http:// 或 https:// 开头的完整网址。
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
