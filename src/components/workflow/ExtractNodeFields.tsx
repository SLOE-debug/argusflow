import type {
  FieldProjectionSource,
  UiOperation,
} from '../../features/workflow/contracts';
import { Input, Select } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';

type ExtractOperation = Extract<UiOperation, { type: 'extract' }>;

type ExtractNodeFieldsProps = Readonly<{
  operation: ExtractOperation;
  onChange: (operation: ExtractOperation) => void;
}>;

/** 提取字段可以读取的内容来源。 */
const PROJECTION_SOURCE_OPTIONS = [
  { value: 'text', label: '显示文字' },
  { value: 'value', label: '控件值' },
  { value: 'name', label: '控件名称' },
  { value: 'property', label: '控件属性' },
  { value: 'attribute', label: '元素属性' },
] as const;

/** 设置要读取多少个目标，以及每个目标要读取哪些内容。 */
export function ExtractNodeFields({
  operation,
  onChange,
}: ExtractNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5 rounded-md border border-cyan-100 bg-cyan-50/40 p-2.5">
      <InspectorField label="读取数量">
        <Select<'one' | 'many'>
          value={operation.cardinality}
          options={[
            { value: 'one', label: '一个' },
            { value: 'many', label: '多个' },
          ]}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(cardinality) => onChange({ ...operation, cardinality })}
        />
      </InspectorField>
      {operation.fields.map((field, index) => (
        <div
          key={`${index}-${field.name}`}
          className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_24px] gap-1.5"
        >
          <Input
            aria-label={`提取字段 ${index + 1} 名称`}
            value={field.name}
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => onChange({
              ...operation,
              fields: operation.fields.map((candidate, fieldIndex) => (
                fieldIndex === index
                  ? { ...candidate, name: event.target.value }
                  : candidate
              )),
            })}
          />
          <Select<FieldProjectionSource['type']>
            value={field.source.type}
            options={PROJECTION_SOURCE_OPTIONS}
            containerClassName="border-slate-300 bg-white"
            onValueChange={(type) => onChange({
              ...operation,
              fields: operation.fields.map((candidate, fieldIndex) => (
                fieldIndex === index
                  ? { ...candidate, source: createProjectionSource(type) }
                  : candidate
              )),
            })}
          />
          <button
            type="button"
            aria-label={`删除提取字段 ${index + 1}`}
            className="h-8 rounded border border-rose-200 text-sm text-rose-600 hover:bg-rose-50 disabled:opacity-40"
            disabled={operation.fields.length === 1}
            onClick={() => onChange({
              ...operation,
              fields: operation.fields.filter((_, fieldIndex) => fieldIndex !== index),
            })}
          >
            ×
          </button>
          {field.source.type === 'property' || field.source.type === 'attribute' ? (
            <Input
              aria-label={`提取字段 ${index + 1} 属性名`}
              value={field.source.name}
              containerClassName="col-span-2 border-slate-300 bg-white"
              placeholder={field.source.type === 'attribute' ? '例如 href' : '例如 checked'}
              onChange={(event) => {
                const source = { ...field.source, name: event.target.value };
                onChange({
                  ...operation,
                  fields: operation.fields.map((candidate, fieldIndex) => (
                    fieldIndex === index ? { ...candidate, source } : candidate
                  )),
                });
              }}
            />
          ) : null}
        </div>
      ))}
      <button
        type="button"
        aria-label="添加字段"
        className="h-7 rounded border border-cyan-200 bg-white text-[10px] font-medium text-cyan-700 hover:bg-cyan-50"
        onClick={() => onChange({
          ...operation,
          fields: [
            ...operation.fields,
            { name: `field_${operation.fields.length + 1}`, source: { type: 'text' } },
          ],
        })}
      >
        添加字段
      </button>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        这里会得到一组数据；需要生成文本或 CSV 时，请使用“整理文本”节点。
      </p>
    </div>
  );
}

/** 为切换后的字段来源创建字段完整的判别联合。 */
function createProjectionSource(type: FieldProjectionSource['type']): FieldProjectionSource {
  switch (type) {
    case 'text':
    case 'value':
    case 'name':
      return { type };
    case 'property':
    case 'attribute':
      return { type, name: '' };
  }
}
