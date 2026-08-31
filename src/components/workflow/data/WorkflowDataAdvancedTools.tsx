import { useEffect, useState, type ReactNode } from 'react';

import { Button, Textarea } from '../../ui';

/** 工作流数据 JSON 高级工具的最小受控契约。 */
export type WorkflowDataAdvancedToolsProps = Readonly<{
  /** 当前数据页签，只展示与当前任务相关的 JSON 工具。 */
  activeSection: 'inputs' | 'variables';
  /** 当前输入声明 JSON 草稿。 */
  inputDefinitionsDraft: string;
  /** 输入声明草稿错误。 */
  inputDefinitionsError: string | null;
  /** 导入输入声明 JSON。 */
  onImportInputs: (draft: string) => boolean;
  /** 导出当前有效输入声明 JSON。 */
  onExportInputs: () => string;
  /** 当前变量 JSON 草稿。 */
  variablesDraft: string;
  /** 变量草稿错误。 */
  variablesError: string | null;
  /** 替换完整变量对象。 */
  onReplaceVariables: (draft: string) => boolean;
}>;

/**
 * 折叠后的开发者 JSON 工具。
 *
 * 默认工作流数据路径只显示结构化 CRUD；只有主动展开时才暴露批量复制和导入入口。
 */
export function WorkflowDataAdvancedTools({
  activeSection,
  inputDefinitionsDraft,
  inputDefinitionsError,
  onImportInputs,
  onExportInputs,
  variablesDraft,
  variablesError,
  onReplaceVariables,
}: WorkflowDataAdvancedToolsProps) {
  const [open, setOpen] = useState(false);
  const [inputDraft, setInputDraft] = useState(inputDefinitionsDraft);
  const [variableDraft, setVariableDraft] = useState(variablesDraft);

  useEffect(() => {
    setInputDraft(inputDefinitionsDraft);
  }, [inputDefinitionsDraft]);

  useEffect(() => {
    setVariableDraft(variablesDraft);
  }, [variablesDraft]);

  return (
    <section className="border-t border-slate-200 pt-3">
      <Button
        variant="ghost"
        size="compact"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        {open ? '收起高级 JSON 工具' : '高级：JSON 工具'}
      </Button>
      {open ? (
        <div className="mt-3 flex flex-col gap-4">
          {activeSection === 'inputs' ? (
            <JsonToolSection
              title="输入声明 JSON"
              description="用于批量复制或与 API 对齐；日常编辑请使用上方表格。"
              value={inputDraft}
              error={inputDefinitionsError}
              onChange={setInputDraft}
              actions={(
                <>
                  <Button
                    size="compact"
                    onClick={() => onImportInputs(inputDraft)}
                  >
                    导入输入声明
                  </Button>
                  <Button
                    variant="secondary"
                    size="compact"
                    onClick={() => setInputDraft(onExportInputs())}
                  >
                    导出当前输入
                  </Button>
                </>
              )}
            />
          ) : (
            <JsonToolSection
              title="工作流变量 JSON"
              description="用于批量复制或调试；日常编辑请使用变量表格。"
              value={variableDraft}
              error={variablesError}
              onChange={setVariableDraft}
              actions={(
                <Button
                  size="compact"
                  onClick={() => onReplaceVariables(variableDraft)}
                >
                  导入变量
                </Button>
              )}
            />
          )}
        </div>
      ) : null}
    </section>
  );
}

/** 一个带说明、错误信息和动作区的 JSON 高级编辑器段落。 */
function JsonToolSection({
  title,
  description,
  value,
  error,
  onChange,
  actions,
}: Readonly<{
  title: string;
  description: string;
  value: string;
  error: string | null;
  onChange: (value: string) => void;
  actions: ReactNode;
}>) {
  return (
    <div className="rounded-md border border-slate-200 bg-slate-50/60 p-3">
      <h3 className="text-[11px] font-semibold text-slate-700">{title}</h3>
      <p className="mt-1 text-[10px] text-slate-500">{description}</p>
      <Textarea
        aria-label={title}
        value={value}
        className="mt-2 h-28 resize-y font-mono text-[10px]"
        onChange={(event) => onChange(event.target.value)}
      />
      {error ? <p className="mt-1 text-[10px] text-rose-600">{error}</p> : null}
      <div className="mt-2 flex flex-wrap justify-end gap-1.5">{actions}</div>
    </div>
  );
}
