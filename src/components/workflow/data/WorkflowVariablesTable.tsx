import Pencil from 'lucide-react/dist/esm/icons/pencil.mjs';
import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import Trash2 from 'lucide-react/dist/esm/icons/trash-2.mjs';
import { useState } from 'react';

import type { JsonObject, JsonValue } from '../../../features/workflow';
import { Button, IconButton, Input, Textarea } from '../../ui';

type WorkflowVariablesTableProps = Readonly<{
  /** 当前工作流变量的初始快照。 */
  variables: JsonObject;
  /** 新建变量；返回 false 表示名称无效或重复。 */
  onAdd: (name: string, value: JsonValue) => boolean;
  /** 修改变量名称和初始值。 */
  onUpdate: (oldName: string, newName: string, value: JsonValue) => boolean;
  /** 删除变量；返回 false 表示仍被节点引用。 */
  onDelete: (name: string) => boolean;
}>;

/** 工作流变量 CRUD；类型只展示 JSON 值推断结果，不改变 schema v9。 */
export function WorkflowVariablesTable({
  variables,
  onAdd,
  onUpdate,
  onDelete,
}: WorkflowVariablesTableProps) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [value, setValue] = useState<JsonValue>('');
  const [error, setError] = useState<string | null>(null);
  const variableEntries = Object.entries(variables);

  const submitAdd = () => {
    const nextName = name.trim();
    if (!nextName || !onAdd(nextName, value)) {
      setError('变量名称不能为空，且不能重复。');
      return;
    }
    setName('');
    setValue('');
    setError(null);
    setAdding(false);
  };

  return (
    <section className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-[13px] font-semibold text-slate-800">工作流变量</h3>
          <p className="mt-1 text-[11px] text-slate-500">这里定义初始值；运行中由“设置变量”节点更新。</p>
        </div>
        <Button
          size="compact"
          icon={Plus}
          onClick={() => {
            setAdding(true);
            setError(null);
          }}
        >
          新建变量
        </Button>
      </div>
      {adding ? (
        <VariableEditor
          name={name}
          value={value}
          error={error}
          onNameChange={setName}
          onValueChange={setValue}
          onSave={submitAdd}
          onCancel={() => setAdding(false)}
        />
      ) : null}
      {variableEntries.length === 0 ? (
        <div className="rounded-md border border-dashed border-slate-300 px-4 py-6 text-center text-[11px] text-slate-500">
          还没有工作流变量。新建变量后，节点里的值选择器会自动列出它。
        </div>
      ) : (
        <div className="overflow-x-auto rounded-md border border-slate-200">
          <table className="w-full min-w-[520px] text-left text-[11px]">
            <thead className="bg-slate-50 text-slate-500">
              <tr>
                <th className="px-3 py-2 font-medium">名称</th>
                <th className="px-3 py-2 font-medium">初始值</th>
                <th className="px-3 py-2 font-medium">类型</th>
                <th className="w-20 px-3 py-2 text-right font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {variableEntries.map(([variableName, variableValue]) => (
                <WorkflowVariableRow
                  key={variableName}
                  name={variableName}
                  value={variableValue}
                  onUpdate={onUpdate}
                  onDelete={onDelete}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function WorkflowVariableRow({
  name,
  value,
  onUpdate,
  onDelete,
}: Readonly<{
  name: string;
  value: JsonValue;
  onUpdate: (oldName: string, newName: string, value: JsonValue) => boolean;
  onDelete: (name: string) => boolean;
}>) {
  const [editing, setEditing] = useState(false);
  const [draftName, setDraftName] = useState(name);
  const [draftValue, setDraftValue] = useState(value);
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    if (!draftName.trim() || !onUpdate(name, draftName.trim(), draftValue)) {
      setError('名称不能为空，且不能重复。');
      return;
    }
    setError(null);
    setEditing(false);
  };

  return (
    <tr className="align-middle text-slate-700">
      <td className="px-3 py-2 font-mono">{name}</td>
      <td className="max-w-[360px] px-3 py-2">
        {editing ? (
          <VariableEditor
            name={draftName}
            value={draftValue}
            error={error}
            inline
            onNameChange={setDraftName}
            onValueChange={setDraftValue}
            onSave={save}
            onCancel={() => setEditing(false)}
          />
        ) : (
          <code className="block max-h-12 overflow-auto whitespace-pre-wrap rounded bg-slate-50 px-2 py-1 font-mono text-[10px] text-slate-600">
            {JSON.stringify(value)}
          </code>
        )}
      </td>
      <td className="px-3 py-2 text-slate-500">{inferValueType(value)}</td>
      <td className="px-3 py-2">
        <div className="flex justify-end gap-1">
          <IconButton
            label={`编辑变量 ${name}`}
            icon={Pencil}
            size="compact"
            onClick={() => {
              setDraftName(name);
              setDraftValue(value);
              setError(null);
              setEditing(true);
            }}
          />
          <IconButton
            label={`删除变量 ${name}`}
            icon={Trash2}
            size="compact"
            className="text-rose-600 hover:bg-rose-50"
            onClick={() => {
              if (!onDelete(name)) {
                setDraftName(name);
                setDraftValue(value);
                setEditing(true);
                setError('该变量仍被节点引用，请先移除引用。');
              }
            }}
          />
        </div>
      </td>
    </tr>
  );
}

function VariableEditor({
  name,
  value,
  error,
  inline = false,
  onNameChange,
  onValueChange,
  onSave,
  onCancel,
}: Readonly<{
  name: string;
  value: JsonValue;
  error: string | null;
  inline?: boolean;
  onNameChange: (name: string) => void;
  onValueChange: (value: JsonValue) => void;
  onSave: () => void;
  onCancel: () => void;
}>) {
  const [draftValue, setDraftValue] = useState(() => JSON.stringify(value, null, 2));
  const [valueError, setValueError] = useState<string | null>(null);
  const updateJson = (nextDraft: string) => {
    setDraftValue(nextDraft);
    try {
      onValueChange(JSON.parse(nextDraft) as JsonValue);
      setValueError(null);
    } catch {
      setValueError('值必须是有效 JSON。');
    }
  };
  return (
    <div className={inline ? 'flex min-w-[300px] flex-col gap-1.5' : 'rounded-md border border-blue-200 bg-blue-50/50 p-2.5'}>
      {!inline ? <p className="text-[11px] font-medium text-slate-600">变量信息</p> : null}
      <div className="flex gap-1.5">
        <Input aria-label="变量名称" value={name} onChange={(event) => onNameChange(event.target.value)} />
        <Textarea
          aria-label="变量初始值"
          className="h-8 resize-none py-1.5 font-mono text-[11px]"
          value={draftValue}
          onChange={(event) => updateJson(event.target.value)}
        />
      </div>
      {valueError || error ? <p className="text-[11px] text-rose-600">{valueError ?? error}</p> : null}
      <div className="flex justify-end gap-1.5">
        <Button size="compact" onClick={onSave} disabled={Boolean(valueError)}>保存</Button>
        <Button size="compact" variant="secondary" onClick={onCancel}>取消</Button>
      </div>
    </div>
  );
}

/** 仅用于展示的 JSON 值类别推断。 */
export function inferValueType(value: JsonValue): string {
  if (value === null) return '空值';
  if (Array.isArray(value)) return '数组';
  switch (typeof value) {
    case 'string': return '文本';
    case 'number': return '数字';
    case 'boolean': return '布尔';
    default: return 'JSON';
  }
}
