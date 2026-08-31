import { useState } from 'react';

import type {
  JsonObject,
  JsonValue,
  WorkflowCanvasNode,
  WorkflowInputDefinition,
} from '../../../features/workflow';
import { getNodeValueOutputs } from '../../../features/workflow';
import { WorkflowInputsTable } from './WorkflowInputsTable';
import { WorkflowNodeOutputsTable } from './WorkflowNodeOutputsTable';
import { WorkflowVariablesTable } from './WorkflowVariablesTable';
import { WorkflowDataAdvancedTools } from './WorkflowDataAdvancedTools';

type WorkflowDataTab = 'inputs' | 'variables' | 'outputs';

type WorkflowDataPanelProps = Readonly<{
  /** 工作流输入声明。 */
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  /** 本次运行输入值。 */
  runInputValues: JsonObject;
  /** 工作流变量初始值。 */
  variables: JsonObject;
  /** 工作流运行期间锁定本次运行输入。 */
  running?: boolean;
  /** 当前画布节点，用于派生节点输出目录。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 新增输入声明。 */
  onAddInput: (key: string) => boolean;
  /** 重命名输入声明。 */
  onRenameInput: (oldKey: string, newKey: string) => boolean;
  /** 删除输入声明。 */
  onDeleteInput: (key: string) => boolean;
  /** 修改运行输入值。 */
  onRunInputValueChange: (key: string, value: string) => void;
  /** 新建变量。 */
  onAddVariable: (name: string, value: JsonValue) => boolean;
  /** 修改变量。 */
  onUpdateVariable: (oldName: string, newName: string, value: JsonValue) => boolean;
  /** 删除变量。 */
  onDeleteVariable: (name: string) => boolean;
  /** 输入声明高级 JSON 草稿。 */
  inputDefinitionsDraft?: string;
  /** 输入声明高级 JSON 错误。 */
  inputDefinitionsError?: string | null;
  /** 导入输入声明 JSON。 */
  onImportInputs?: (draft: string) => boolean;
  /** 导出输入声明 JSON。 */
  onExportInputs?: () => string;
  /** 工作流变量高级 JSON 草稿。 */
  variablesDraft?: string;
  /** 工作流变量高级 JSON 错误。 */
  variablesError?: string | null;
  /** 替换完整变量 JSON。 */
  onReplaceVariables?: (draft: string) => boolean;
}>;

/** 工作流级的统一数据面板；节点输出只读，输入和变量可编辑。 */
export function WorkflowDataPanel({
  inputs,
  runInputValues,
  variables,
  running = false,
  nodes,
  onAddInput,
  onRenameInput,
  onDeleteInput,
  onRunInputValueChange,
  onAddVariable,
  onUpdateVariable,
  onDeleteVariable,
  inputDefinitionsDraft,
  inputDefinitionsError,
  onImportInputs,
  onExportInputs,
  variablesDraft,
  variablesError,
  onReplaceVariables,
}: WorkflowDataPanelProps) {
  const [activeTab, setActiveTab] = useState<WorkflowDataTab>('inputs');
  const tabs = [
    { id: 'inputs' as const, label: `输入参数 ${inputs.length}` },
    { id: 'variables' as const, label: `工作流变量 ${Object.keys(variables).length}` },
    {
      id: 'outputs' as const,
      label: `节点输出 ${nodes.reduce((count, node) => count + getNodeValueOutputs(node.data).length, 0)}`,
    },
  ];
  return (
    <section className="flex min-h-0 flex-1 flex-col bg-white">
      <header className="flex shrink-0 items-center border-b border-slate-200 px-4 py-2.5">
        <div>
          <h2 className="text-[14px] font-semibold text-slate-900">工作流数据</h2>
          <p className="mt-0.5 text-[11px] text-slate-500">输入、变量和节点输出共享一个值空间。</p>
        </div>
      </header>
      <nav className="flex shrink-0 gap-1 border-b border-slate-200 px-3" aria-label="工作流数据类别">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className={`relative h-9 px-2.5 text-[11px] ${activeTab === tab.id
              ? 'font-semibold text-blue-600 after:absolute after:inset-x-1 after:bottom-0 after:h-0.5 after:bg-blue-600'
              : 'text-slate-500 hover:text-slate-800'}`}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </nav>
      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {activeTab === 'inputs' && inputDefinitionsError ? (
          <p role="alert" className="mb-3 rounded-md bg-rose-50 px-3 py-2 text-[11px] text-rose-700">
            {inputDefinitionsError}
          </p>
        ) : null}
        {activeTab === 'variables' && variablesError ? (
          <p role="alert" className="mb-3 rounded-md bg-rose-50 px-3 py-2 text-[11px] text-rose-700">
            {variablesError}
          </p>
        ) : null}
        {activeTab === 'inputs' ? (
          <WorkflowInputsTable
            inputs={inputs}
            runValues={runInputValues}
            onAdd={onAddInput}
            onRename={onRenameInput}
            onDelete={onDeleteInput}
            onRunValueChange={onRunInputValueChange}
            runLocked={running}
          />
        ) : null}
        {activeTab === 'variables' ? (
          <WorkflowVariablesTable
            variables={variables}
            onAdd={onAddVariable}
            onUpdate={onUpdateVariable}
            onDelete={onDeleteVariable}
          />
        ) : null}
        {activeTab === 'outputs' ? <WorkflowNodeOutputsTable nodes={nodes} /> : null}
        {activeTab !== 'outputs'
          && inputDefinitionsDraft !== undefined
          && onImportInputs
          && onExportInputs
          && variablesDraft !== undefined
          && onReplaceVariables ? (
          <WorkflowDataAdvancedTools
            activeSection={activeTab}
            inputDefinitionsDraft={inputDefinitionsDraft}
            inputDefinitionsError={inputDefinitionsError ?? null}
            onImportInputs={onImportInputs}
            onExportInputs={onExportInputs}
            variablesDraft={variablesDraft}
            variablesError={variablesError ?? null}
            onReplaceVariables={onReplaceVariables}
          />
        ) : null}
      </div>
    </section>
  );
}
