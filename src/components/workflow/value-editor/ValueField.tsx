import { useEffect, useState, type ReactNode } from 'react';

import type {
  JsonValue,
  ValueExpr,
  WorkflowSymbolRegistry,
} from '../../../features/workflow';
import { Button, Input, Textarea } from '../../ui';
import type { ValueExprLocation } from '../../../features/workflow';
import { ValuePicker } from './ValuePicker';

/** 字符串常量可以使用普通输入框或业务专属编辑器。 */
export type ValueLiteralPresentation =
  | { type: 'single_line' }
  | {
      type: 'custom';
      /** 使用业务专属编辑器渲染字符串常量。 */
      render: (props: ValueLiteralEditorProps) => ReactNode;
    };

/** 业务专属字符串常量编辑器获得的最小受控契约。 */
export type ValueLiteralEditorProps = Readonly<{
  label: string;
  value: string;
  onChange: (value: string) => void;
}>;

type ValueFieldProps = Readonly<{
  /** 字段的人类可读名称。 */
  label: string;
  /** 当前完整值表达式。 */
  value: ValueExpr;
  /** 写回值表达式。 */
  onChange: (value: ValueExpr) => void;
  /** 当前工作流值目录。 */
  symbols?: WorkflowSymbolRegistry;
  /** JSON 字面量边界使用完整 JSON 编辑。 */
  literalMode?: 'text' | 'json';
  /** 文本常量的业务专属展示方式。 */
  literalPresentation?: ValueLiteralPresentation;
  /** 是否允许打开受限 Rhai 高级编辑器。 */
  allowExpression?: boolean;
  /** 中央 Workspace 的表达式目标；提供后显示打开按钮。 */
  expressionLocation?: ValueExprLocation;
  /** 请求中央 Workspace 打开表达式。 */
  onOpenExpression?: (location: ValueExprLocation) => void;
}>;

/** 统一编辑常量、已有值引用和受限 Rhai 表达式的字段。 */
export function ValueField({
  label,
  value,
  onChange,
  symbols,
  literalMode = 'text',
  literalPresentation = { type: 'single_line' },
  allowExpression = false,
  expressionLocation,
  onOpenExpression,
}: ValueFieldProps) {
  const switchToLiteral = () => onChange({ type: 'literal', value: literalMode === 'json' ? null : '' });
  return (
    <div className="flex flex-col gap-1.5 text-[11px] font-medium text-slate-600">
      <span>{label}</span>
      {value.type === 'literal'
        && literalMode === 'text'
        && literalPresentation.type === 'single_line' ? (
        <Input
          aria-label={label}
          value={typeof value.value === 'string' ? value.value : ''}
          onChange={(event) => onChange({ type: 'literal', value: event.target.value })}
        />
      ) : null}
      {value.type === 'literal'
        && literalMode === 'text'
        && literalPresentation.type === 'custom'
        ? literalPresentation.render({
            label,
            value: typeof value.value === 'string' ? value.value : '',
            onChange: (nextValue) => onChange({ type: 'literal', value: nextValue }),
          })
        : null}
      {value.type === 'literal' && literalMode === 'json' ? (
        <JsonLiteralInput label={label} value={value.value} onChange={(next) => onChange({ type: 'literal', value: next })} />
      ) : null}
      {value.type !== 'expression' && symbols ? (
        <ValuePicker value={value} symbols={symbols} onChange={onChange} ariaLabel={`${label}：选择工作流值`} />
      ) : null}
      {value.type === 'expression' ? (
        <div className="flex items-center gap-2 rounded-md border border-blue-200 bg-blue-50/50 px-2.5 py-2">
          <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-slate-700">{value.source || '还没有填写表达式'}</code>
          {expressionLocation && onOpenExpression ? (
            <Button size="compact" onClick={() => onOpenExpression(expressionLocation)}>编辑</Button>
          ) : null}
        </div>
      ) : null}
      <div className="flex items-center gap-1.5">
        {value.type !== 'literal' ? (
          <Button
            size="compact"
            variant="secondary"
            onClick={switchToLiteral}
          >
            使用常量
          </Button>
        ) : null}
        {value.type !== 'expression' && allowExpression ? (
          <Button
            size="compact"
            variant="secondary"
            onClick={() => {
              onChange({ type: 'expression', source: '' });
            }}
          >
            fx 高级表达式
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function JsonLiteralInput({
  label,
  value,
  onChange,
}: Readonly<{ label: string; value: JsonValue; onChange: (value: JsonValue) => void }>) {
  const [draft, setDraft] = useState(() => JSON.stringify(value, null, 2));
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setDraft(JSON.stringify(value, null, 2));
    setError(null);
  }, [value]);
  return (
    <span>
      <Textarea
        aria-label={label}
        value={draft}
        className="h-20 resize-y font-mono text-[11px]"
        onChange={(event) => {
          setDraft(event.target.value);
          try {
            onChange(JSON.parse(event.target.value) as JsonValue);
            setError(null);
          } catch {
            setError('值必须是有效 JSON。');
          }
        }}
      />
      {error ? <span className="mt-1 block text-[10px] font-normal text-rose-600">{error}</span> : null}
    </span>
  );
}
