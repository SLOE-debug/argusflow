import type { WorkflowNodeData, WorkflowNodeUpdater } from '../../../../features/workflow';
import { Input } from '../../../ui';
import { InspectorField } from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type FailData = Extract<WorkflowNodeData, { kind: 'fail' }>;

/** 编辑显式失败终点的稳定错误码与运行时消息。 */
export function FailNodeFields({
  data,
  onUpdate,
}: Readonly<{ data: FailData; onUpdate: (updater: WorkflowNodeUpdater) => void }>) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="错误标识">
        <Input
          value={data.code}
          onChange={(event) => {
            const code = event.target.value;
            onUpdate((current) => current.kind === 'fail'
              ? { ...current, code, invalid: false }
              : current);
          }}
        />
      </InspectorField>
      <p className="text-[10px] leading-4 text-slate-500">
        使用简短且稳定的名称，例如 contact_not_found。
      </p>
      <ValueExprFields
        value={data.message}
        literalLabel="给用户的说明"
        expressionLocation={{ type: 'fail_message' }}
        onChange={(message) => onUpdate((current) => current.kind === 'fail'
          ? { ...current, message, invalid: false }
          : current)}
      />
    </div>
  );
}
