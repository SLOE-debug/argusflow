import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
  InspectorSection,
} from './InspectorControls';

type WorkflowInspectorFieldsProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** JSON 变量编辑草稿。 */
  variablesDraft: string;
  /** JSON 草稿的即时错误。 */
  variablesError: string | null;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 修改 JSON 变量草稿。 */
  onVariablesChange: (draft: string) => void;
}>;

/** 工作流级基本信息和 JSON 变量设置。 */
export function WorkflowInspectorFields({
  workflowName,
  variablesDraft,
  variablesError,
  onNameChange,
  onVariablesChange,
}: WorkflowInspectorFieldsProps) {
  const formatVariables = () => {
    onVariablesChange(JSON.stringify(JSON.parse(variablesDraft), null, 2));
  };

  return (
    <>
      <InspectorSection title="基本信息">
        <InspectorField label="流程名称">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={workflowName}
            onChange={(event) => onNameChange(event.target.value)}
          />
        </InspectorField>
        <InspectorField label="流程 ID">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value="workflow_sync_01"
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="流程变量">
        <textarea
          className={`${INSPECTOR_CONTROL_CLASS_NAME} h-[190px] resize-none py-2 font-mono leading-5`}
          spellCheck={false}
          value={variablesDraft}
          onChange={(event) => onVariablesChange(event.target.value)}
        />
        {variablesError ? (
          <p className="text-[11px] leading-4 text-rose-600">{variablesError}</p>
        ) : (
          <button
            type="button"
            className="flex h-8 items-center justify-center self-start rounded-[4px] border border-slate-300 bg-white px-3 text-[11px] text-slate-600 hover:bg-slate-50"
            onClick={formatVariables}
          >
            格式化 JSON
          </button>
        )}
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          条件节点使用 RFC 6901 JSON Pointer 读取这里的值。
        </p>
      </InspectorSection>
    </>
  );
}
