import { useEffect, useState } from 'react';

import type { JsonObject, WorkflowInputDefinition } from '../../../features/workflow';
import {
  normalizeRunInputValues,
  validateRunInputValues,
} from '../../../features/workflow';
import { Button, Dialog, Input, Textarea } from '../../ui';

type RunInputsDialogProps = Readonly<{
  /** 当前是否打开运行表单。 */
  open: boolean;
  /** 工作流输入声明。 */
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  /** 当前运行输入值。 */
  values: JsonObject;
  /** 关闭运行表单。 */
  onOpenChange: (open: boolean) => void;
  /** 保存一份运行输入并启动工作流。 */
  onSubmit: (values: JsonObject) => void;
}>;

/** 根据输入声明自动生成运行表单；高级 JSON 仅作为折叠工具保留。 */
export function RunInputsDialog({
  open,
  inputs,
  values,
  onOpenChange,
  onSubmit,
}: RunInputsDialogProps) {
  const [draft, setDraft] = useState<JsonObject>(values);
  const [advanced, setAdvanced] = useState(false);
  const [advancedDraft, setAdvancedDraft] = useState(() => JSON.stringify(values, null, 2));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const normalizedValues = normalizeRunInputValues(inputs, values);
    setDraft(normalizedValues);
    setAdvancedDraft(JSON.stringify(normalizedValues, null, 2));
    setError(null);
    setAdvanced(false);
  }, [inputs, open, values]);

  const updateField = (key: string, value: string) => {
    const next = { ...draft, [key]: value };
    setDraft(next);
    setAdvancedDraft(JSON.stringify(next, null, 2));
  };
  const submit = () => {
    try {
      const submitted: unknown = advanced ? JSON.parse(advancedDraft) : draft;
      const validation = validateRunInputValues(inputs, submitted);
      if (!validation.valid) {
        setError(validation.message);
        return;
      }
      onSubmit(validation.values);
    } catch {
      setError('JSON 格式有误，请检查引号、括号和逗号。');
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="运行工作流"
      description="填写本次运行的输入参数。运行开始后，这些值不会写回工作流。"
      footer={(
        <>
          <Button variant="secondary" onClick={() => onOpenChange(false)}>取消</Button>
          <Button onClick={submit}>运行工作流</Button>
        </>
      )}
    >
      <div className="flex flex-col gap-3">
        {inputs.length === 0 ? (
          <p className="rounded-md bg-slate-50 px-3 py-2 text-[11px] text-slate-500">这个工作流不需要输入参数。</p>
        ) : (
          inputs.map((input) => (
            <label key={input.key} className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
              {input.key}
              <Input
                data-dialog-initial-focus={input === inputs[0] ? true : undefined}
                value={typeof draft[input.key] === 'string' ? String(draft[input.key]) : ''}
                onChange={(event) => updateField(input.key, event.target.value)}
              />
            </label>
          ))
        )}
        <Button
          variant="ghost"
          size="compact"
          className="self-start px-0 text-[11px] text-blue-600 hover:bg-transparent hover:text-blue-700"
          onClick={() => setAdvanced((current) => !current)}
          aria-expanded={advanced}
        >
          {advanced ? '收起高级 JSON' : '高级：JSON 编辑'}
        </Button>
        {advanced ? (
          <Textarea
            aria-label="本次运行输入 JSON"
            value={advancedDraft}
            className="h-32 resize-y font-mono text-[11px]"
            onChange={(event) => {
              setAdvancedDraft(event.target.value);
              setError(null);
            }}
          />
        ) : null}
        {error ? <p role="alert" className="text-[11px] text-rose-600">{error}</p> : null}
      </div>
    </Dialog>
  );
}
