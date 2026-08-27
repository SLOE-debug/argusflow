import type {
  ValueExpr,
  ValueExprKind,
} from '../../features/workflow/contracts';
import type { ReactNode } from 'react';

import { Input, Select } from '../ui';
import { InspectorField } from './InspectorControls';

/** ValueExpr 字面量与业务语义解耦后的展示方式。 */
export type LiteralPresentation =
  | { type: 'single_line' }
  | {
      type: 'custom';
      /** 使用业务专属编辑器渲染字面量，引用来源仍由本组件处理。 */
      render: (props: LiteralEditorProps) => ReactNode;
    };

/** 业务专属字面量编辑器获得的最小受控契约。 */
export type LiteralEditorProps = Readonly<{
  /** 字段可访问名称。 */
  label: string;
  /** 当前字符串字面量。 */
  value: string;
  /** 原样写回完整字符串。 */
  onChange: (value: string) => void;
}>;

type ValueExprFieldsProps = Readonly<{
  /** 当前要求最终解析为字符串的值表达式。 */
  value: ValueExpr;
  /** 写回字段完整的新表达式。 */
  onChange: (value: ValueExpr) => void;
  /** 字面量输入框使用的可访问名称。 */
  literalLabel?: string;
  /** 字面量的展示方式；引用来源不受该配置影响。 */
  literalPresentation?: LiteralPresentation;
}>;

/** 工作流值来源选项。 */
const VALUE_KIND_OPTIONS = [
  { value: 'literal', label: '固定文本' },
  { value: 'workflow_input', label: '工作流输入' },
  { value: 'node_output', label: '节点输出' },
  { value: 'variable', label: '运行变量' },
] as const;

/** 编辑最终必须解析为字符串的 ValueExpr。 */
export function ValueExprFields({
  value,
  onChange,
  literalLabel = '固定文本',
  literalPresentation = { type: 'single_line' },
}: ValueExprFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5 rounded-md border border-slate-200 bg-slate-50/60 p-2.5">
      <InspectorField label="数据来源">
        <Select<ValueExprKind>
          value={value.type}
          options={VALUE_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(createValueExpr(kind))}
        />
      </InspectorField>
      {value.type === 'literal' ? (
        <LiteralField
          label={literalLabel}
          value={typeof value.value === 'string' ? value.value : ''}
          presentation={literalPresentation}
          onChange={(literalValue) => onChange({
            type: 'literal',
            value: literalValue,
          })}
        />
      ) : null}
      {value.type === 'workflow_input' ? (
        <InspectorField label="输入字段">
          <Input
            aria-label="工作流输入字段"
            value={value.key}
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => onChange({ ...value, key: event.target.value })}
          />
        </InspectorField>
      ) : null}
      {value.type === 'node_output' ? (
        <>
          <InspectorField label="生产节点 ID">
            <Input
              aria-label="输出生产节点 ID"
              value={value.node_id}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onChange({ ...value, node_id: event.target.value })}
            />
          </InspectorField>
          <InspectorField label="输出端口">
            <Input
              aria-label="节点输出端口"
              value={value.output}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onChange({ ...value, output: event.target.value })}
            />
          </InspectorField>
        </>
      ) : null}
      {value.type === 'variable' ? (
        <InspectorField label="变量名称">
          <Input
            aria-label="运行变量名称"
            value={value.name}
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => onChange({ ...value, name: event.target.value })}
          />
        </InspectorField>
      ) : null}
    </div>
  );
}

type LiteralFieldProps = Readonly<{
  label: string;
  value: string;
  presentation: LiteralPresentation;
  onChange: (value: string) => void;
}>;

/** 根据字段语义选择通用单行控件或业务专属字面量编辑器。 */
function LiteralField({
  label,
  value,
  presentation,
  onChange,
}: LiteralFieldProps) {
  if (presentation.type === 'single_line') {
    return (
      <InspectorField label={label}>
        <Input
          aria-label={label}
          value={value}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange(event.target.value)}
        />
      </InspectorField>
    );
  }

  return presentation.render({ label, value, onChange });
}

/** 切换数据来源时建立字段完整的新表达式。 */
export function createValueExpr(kind: ValueExprKind): ValueExpr {
  switch (kind) {
    case 'literal':
      return { type: kind, value: '' };
    case 'workflow_input':
      return { type: kind, key: '' };
    case 'node_output':
      return { type: kind, node_id: '', output: '' };
    case 'variable':
      return { type: kind, name: '' };
  }
}
