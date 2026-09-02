import type {
  UiExecutionPolicy,
  UiOperation,
  UiOperationKind,
} from '../../../../features/workflow';
import {
  changeKeyChord,
  changeSetValue,
  changeTypeText,
  changeUiOperationKind,
  createTargetWaitPolicy,
} from '../../../../features/workflow';
import { Select } from '../../../ui';
import { InspectorField, InspectorSection } from '../InspectorControls';
import { KeyboardChordFields } from '../node-fields/KeyboardChordFields';
import { ValueExprFields } from '../node-fields/ValueExprFields';

type ActionSectionProps = Readonly<{
  /** 当前完整动作。 */
  operation: UiOperation;
  /** 动作切换定位语义时同步更新的等待策略。 */
  execution: UiExecutionPolicy;
  /** 写回新动作。 */
  onOperationChange: (operation: UiOperation) => void;
  /** 写回新执行预算。 */
  onExecutionChange: (execution: UiExecutionPolicy) => void;
}>;

/** 普通用户可以选择的任务动作。 */
const ACTION_OPTIONS = [
  { value: 'click', label: '单击' },
  { value: 'set_value', label: '输入文本' },
  { value: 'press_key', label: '按键' },
  { value: 'type_text', label: '键入文本' },
] as const;

/** 只回答“做什么”的动作区块。 */
export function ActionSection({
  operation,
  execution,
  onOperationChange,
  onExecutionChange,
}: ActionSectionProps) {
  return (
    <InspectorSection title="做什么">
      <InspectorField label="动作">
        <Select<UiOperationKind>
          aria-label="动作"
          value={operation.type}
          options={ACTION_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => {
            const nextOperation = changeUiOperationKind(operation, kind);
            onOperationChange(nextOperation);
            if (nextOperation.target.locator.type !== operation.target.locator.type) {
              onExecutionChange({
                ...execution,
                target_wait: createTargetWaitPolicy(nextOperation.target.locator.type),
              });
            }
          }}
        />
      </InspectorField>
      {operation.type === 'set_value' ? (
        <ValueExprFields
          value={operation.value}
          literalLabel="输入内容"
          expressionLocation={{ type: 'ui_set_value' }}
          onChange={(value) => onOperationChange(changeSetValue(operation, value))}
        />
      ) : null}
      {operation.type === 'type_text' ? (
        <ValueExprFields
          value={operation.value}
          literalLabel="输入内容"
          expressionLocation={{ type: 'ui_type_text' }}
          onChange={(value) => onOperationChange(changeTypeText(operation, value))}
        />
      ) : null}
      {operation.type === 'press_key' ? (
        <KeyboardChordFields
          chord={operation.chord}
          onChange={(chord) => onOperationChange(changeKeyChord(operation, chord))}
        />
      ) : null}
    </InspectorSection>
  );
}
