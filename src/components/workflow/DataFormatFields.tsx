import type { DelimitedTextFormat } from '../../features/workflow/contracts';
import type { WorkflowNodeUpdater } from '../../features/workflow/workflowModel';
import { Checkbox, Input } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type DataFormatFieldsProps = Readonly<{
  operation: DelimitedTextFormat;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 编辑对象数组到分隔文本的确定格式。 */
export function DataFormatFields({ operation, onUpdate }: DataFormatFieldsProps) {
  const change = (next: DelimitedTextFormat) => onUpdate((current) => (
    current.kind === 'format'
      ? { ...current, operation: next, invalid: false }
      : current
  ));
  return (
    <div className="flex flex-col gap-2.5">
      <ValueExprFields
        value={operation.items}
        literalLabel="对象数组"
        literalMode="json"
        onChange={(items) => change({ ...operation, items })}
      />
      <InspectorField label="字段顺序">
        <Input
          aria-label="格式化字段顺序"
          value={operation.fields.join(', ')}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => change({
            ...operation,
            fields: event.target.value.split(',').map((field) => field.trim()),
          })}
        />
      </InspectorField>
      <InspectorField label="列分隔符">
        <Input
          aria-label="列分隔符"
          value={escapeSeparator(operation.column_separator)}
          containerClassName="border-slate-300 bg-white font-mono"
          onChange={(event) => change({
            ...operation,
            column_separator: unescapeSeparator(event.target.value),
          })}
        />
      </InspectorField>
      <InspectorField label="行分隔符">
        <Input
          aria-label="行分隔符"
          value={escapeSeparator(operation.row_separator)}
          containerClassName="border-slate-300 bg-white font-mono"
          onChange={(event) => change({
            ...operation,
            row_separator: unescapeSeparator(event.target.value),
          })}
        />
      </InspectorField>
      <label className="flex items-center gap-2 text-[11px] text-slate-700">
        <Checkbox
          aria-label="包含字段标题"
          checked={operation.include_header}
          onChange={(event) => change({
            ...operation,
            include_header: event.target.checked,
          })}
        />
        包含字段标题
      </label>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        分隔符编辑器使用 \t、\r、\n 可见转义，运行时输出真实控制字符。
      </p>
    </div>
  );
}

/** 把控制字符转换成单行编辑器可见文本。 */
function escapeSeparator(value: string): string {
  return value.replaceAll('\t', '\\t').replaceAll('\r', '\\r').replaceAll('\n', '\\n');
}

/** 把受支持的可见转义还原为控制字符。 */
function unescapeSeparator(value: string): string {
  return value.replaceAll('\\t', '\t').replaceAll('\\r', '\r').replaceAll('\\n', '\n');
}
