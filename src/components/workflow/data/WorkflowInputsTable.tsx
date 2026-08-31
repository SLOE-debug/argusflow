import Pencil from 'lucide-react/dist/esm/icons/pencil.mjs';
import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import Trash2 from 'lucide-react/dist/esm/icons/trash-2.mjs';
import { useState } from 'react';

import type {
  JsonObject,
  WorkflowInputDefinition,
} from '../../../features/workflow';
import { Button, IconButton, Input } from '../../ui';

type WorkflowInputsTableProps = Readonly<{
  /** 当前工作流声明的输入参数。 */
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  /** 本次运行为输入参数保存的瞬时值。 */
  runValues: JsonObject;
  /** 新增一个输入声明；返回 false 表示名称无效或重复。 */
  onAdd: (key: string) => boolean;
  /** 修改输入声明名称。 */
  onRename: (oldKey: string, newKey: string) => boolean;
  /** 删除输入声明。 */
  onDelete: (key: string) => boolean;
  /** 修改本次运行的一个输入值。 */
  onRunValueChange: (key: string, value: string) => void;
  /** 工作流运行期间锁定本次运行输入。 */
  runLocked?: boolean;
}>;

/** 以表格形式编辑输入声明，并直接展示本次运行的值。 */
export function WorkflowInputsTable({
  inputs,
  runValues,
  onAdd,
  onRename,
  onDelete,
  onRunValueChange,
  runLocked = false,
}: WorkflowInputsTableProps) {
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);

  const submitAdd = () => {
    const key = draft.trim();
    if (!key || !onAdd(key)) {
      setError('输入参数名称不能为空，且不能重复。');
      return;
    }
    setDraft('');
    setError(null);
    setAdding(false);
  };

  return (
    <section className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-[13px] font-semibold text-slate-800">输入参数</h3>
          <p className="mt-1 text-[11px] text-slate-500">运行前填写，执行开始后保持只读。</p>
        </div>
        <Button
          size="compact"
          icon={Plus}
          disabled={runLocked}
          onClick={() => {
            setAdding(true);
            setError(null);
          }}
        >
          添加输入
        </Button>
      </div>
      {adding ? (
        <div className="rounded-md border border-blue-200 bg-blue-50/50 p-2.5">
          <label className="block text-[11px] font-medium text-slate-600" htmlFor="workflow-input-key">
            参数名称
          </label>
          <div className="mt-1.5 flex gap-2">
            <Input
              id="workflow-input-key"
              autoFocus
              value={draft}
              placeholder="例如 contact_name"
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') submitAdd();
                if (event.key === 'Escape') setAdding(false);
              }}
            />
            <Button size="compact" disabled={runLocked} onClick={submitAdd}>保存</Button>
            <Button size="compact" variant="secondary" onClick={() => setAdding(false)}>取消</Button>
          </div>
          {error ? <p className="mt-1.5 text-[11px] text-rose-600">{error}</p> : null}
        </div>
      ) : null}
      {inputs.length === 0 ? (
        <div className="rounded-md border border-dashed border-slate-300 px-4 py-6 text-center text-[11px] text-slate-500">
          还没有输入参数。添加一个参数，运行时就会自动出现对应输入框。
        </div>
      ) : (
        <div className="overflow-x-auto rounded-md border border-slate-200">
          <table className="w-full min-w-[520px] text-left text-[11px]">
            <thead className="bg-slate-50 text-slate-500">
              <tr>
                <th className="px-3 py-2 font-medium">名称</th>
                <th className="px-3 py-2 font-medium">类型</th>
                <th className="px-3 py-2 font-medium">本次运行</th>
                <th className="w-20 px-3 py-2 text-right font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {inputs.map((input) => (
                <WorkflowInputRow
                  key={input.key}
                  input={input}
                  runValue={typeof runValues[input.key] === 'string' ? String(runValues[input.key]) : ''}
                  onRename={onRename}
                  onDelete={onDelete}
                  onRunValueChange={onRunValueChange}
                  runLocked={runLocked}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function WorkflowInputRow({
  input,
  runValue,
  onRename,
  onDelete,
  onRunValueChange,
  runLocked,
}: Readonly<{
  input: WorkflowInputDefinition;
  runValue: string;
  onRename: (oldKey: string, newKey: string) => boolean;
  onDelete: (key: string) => boolean;
  onRunValueChange: (key: string, value: string) => void;
  runLocked: boolean;
}>) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(input.key);
  const [error, setError] = useState<string | null>(null);
  const saveRename = () => {
    const next = draft.trim();
    if (!next || !onRename(input.key, next)) {
      setError('名称不能为空，且不能重复。');
      return;
    }
    setError(null);
    setEditing(false);
  };

  return (
    <tr className="align-middle text-slate-700">
      <td className="px-3 py-2">
        {editing ? (
          <div className="flex items-center gap-1.5">
            <Input
              aria-label={`编辑输入参数 ${input.key}`}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') saveRename();
                if (event.key === 'Escape') setEditing(false);
              }}
            />
            {error ? <span className="text-rose-600">{error}</span> : null}
          </div>
        ) : (
          <span className="font-mono">{input.key}</span>
        )}
      </td>
      <td className="px-3 py-2 text-slate-500">文本</td>
      <td className="px-3 py-2">
        <Input
          aria-label={`${input.key} 本次运行值`}
          value={runValue}
          disabled={runLocked}
          onChange={(event) => onRunValueChange(input.key, event.target.value)}
          placeholder="运行时填写"
        />
      </td>
      <td className="px-3 py-2">
        <div className="flex justify-end gap-1">
          {editing ? (
            <Button size="compact" onClick={saveRename}>保存</Button>
          ) : (
            <IconButton
              label={`编辑输入参数 ${input.key}`}
              icon={Pencil}
              size="compact"
              disabled={runLocked}
              onClick={() => {
                setDraft(input.key);
                setError(null);
                setEditing(true);
              }}
            />
          )}
          <IconButton
            label={`删除输入参数 ${input.key}`}
            icon={Trash2}
            size="compact"
            disabled={runLocked}
            className="text-rose-600 hover:bg-rose-50"
            onClick={() => {
              if (!onDelete(input.key)) {
                setDraft(input.key);
                setEditing(true);
                setError('该输入仍被节点引用，请先移除引用。');
              }
            }}
          />
        </div>
      </td>
    </tr>
  );
}
