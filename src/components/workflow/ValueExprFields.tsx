import {
  createContext,
  useContext,
  useEffect,
  useId,
  useState,
  type ReactNode,
} from 'react';

import type {
  JsonValue,
  ValueExpr,
  ValueExprKind,
  ValueSource,
  WorkflowInputDefinition,
} from '../../features/workflow/contracts';
import { getNodeValueOutputs } from '../../features/workflow/workflowNodeDefinitions';
import type {
  WorkflowCanvasNode,
} from '../../features/workflow/workflowModel';
import type { ValueExprLocation } from '../../features/workflow/workflowValueExpressions';
import { Input, Select } from '../ui';
import { InspectorField } from './InspectorControls';

/** ValueExpr 字面量与业务语义解耦后的展示方式。 */
export type LiteralPresentation =
  | { type: 'single_line' }
  | {
      type: 'custom';
      /** 使用业务专属编辑器渲染字符串字面量。 */
      render: (props: LiteralEditorProps) => ReactNode;
    };

/** 业务专属字符串字面量编辑器获得的最小受控契约。 */
export type LiteralEditorProps = Readonly<{
  label: string;
  value: string;
  onChange: (value: string) => void;
}>;

type ValueExprFieldsProps = Readonly<{
  /** 当前完整值表达式。 */
  value: ValueExpr;
  /** 写回字段完整的新表达式。 */
  onChange: (value: ValueExpr) => void;
  /** 字面量输入框使用的可访问名称。 */
  literalLabel?: string;
  /** 文本消费边界使用字符串，JSON 数据边界使用完整 JSON 编辑。 */
  literalMode?: 'text' | 'json';
  /** 字符串字面量的业务专属展示方式。 */
  literalPresentation?: LiteralPresentation;
  /** 中央 Workspace 定位当前表达式字段所需的稳定路径。 */
  expressionLocation?: ValueExprLocation;
}>;

type ValueExprEditorContextValue = Readonly<{
  /** 当前节点之前可由用户选择的上游节点。 */
  upstreamNodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 工作流声明的运行输入。 */
  workflowInputs: ReadonlyArray<WorkflowInputDefinition>;
  /** 初始变量对象中的可选字段名。 */
  variableNames: ReadonlyArray<string>;
  /** 请求中央 Workspace 打开指定表达式。 */
  onOpenExpression: (location: ValueExprLocation) => void;
}>;

const ValueExprEditorContext = createContext<ValueExprEditorContextValue | null>(null);

/** 为一个节点的全部 ValueExpr 编辑器提供引用候选与 Workspace 路由。 */
export function ValueExprEditorProvider({
  value,
  children,
}: Readonly<{ value: ValueExprEditorContextValue; children: ReactNode }>) {
  return (
    <ValueExprEditorContext.Provider value={value}>
      {children}
    </ValueExprEditorContext.Provider>
  );
}

/** 顶层来源模式固定为字面量、结构化引用和高级表达式。 */
const VALUE_KIND_OPTIONS = [
  { value: 'literal', label: '固定值' },
  { value: 'ref', label: '引用' },
  { value: 'expression', label: '表达式' },
] as const;

/** 结构化引用的三种稳定来源。 */
const SOURCE_KIND_OPTIONS = [
  { value: 'node', label: '节点' },
  { value: 'variable', label: '运行变量' },
  { value: 'workflow_input', label: '工作流输入' },
] as const;

/** 编辑可以解析成任意 JSON 值的 ValueExpr V2。 */
export function ValueExprFields({
  value,
  onChange,
  literalLabel = '固定值',
  literalMode = 'text',
  literalPresentation = { type: 'single_line' },
  expressionLocation,
}: ValueExprFieldsProps) {
  const editorContext = useContext(ValueExprEditorContext);
  return (
    <div className="flex flex-col gap-2.5 rounded-md border border-slate-200 bg-slate-50/60 p-2.5">
      <InspectorField label="数据来源">
        <Select<ValueExprKind>
          aria-label="值表达式模式"
          value={value.type}
          options={VALUE_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(createValueExpr(kind))}
        />
      </InspectorField>
      {value.type === 'literal' ? (
        literalMode === 'json' ? (
          <JsonLiteralField
            label={literalLabel}
            value={value.value}
            onChange={(literalValue) => onChange({
              type: 'literal',
              value: literalValue,
            })}
          />
        ) : (
          <LiteralField
            label={literalLabel}
            value={typeof value.value === 'string' ? value.value : ''}
            presentation={literalPresentation}
            onChange={(literalValue) => onChange({
              type: 'literal',
              value: literalValue,
            })}
          />
        )
      ) : null}
      {value.type === 'ref' ? (
        <ReferenceFields
          value={value}
          editorContext={editorContext}
          onChange={onChange}
        />
      ) : null}
      {value.type === 'expression' ? (
        <ExpressionSummary
          source={value.source}
          canOpen={Boolean(editorContext && expressionLocation)}
          onOpen={() => {
            if (editorContext && expressionLocation) {
              editorContext.onOpenExpression(expressionLocation);
            }
          }}
        />
      ) : null}
    </div>
  );
}

type ReferenceFieldsProps = Readonly<{
  value: Extract<ValueExpr, { type: 'ref' }>;
  editorContext: ValueExprEditorContextValue | null;
  onChange: (value: ValueExpr) => void;
}>;

/** 引用模式只暴露选择器与 JSON Pointer，不要求用户输入内部节点 ID。 */
function ReferenceFields({ value, editorContext, onChange }: ReferenceFieldsProps) {
  const sourceType = value.source.type;
  return (
    <>
      <InspectorField label="来源类型">
        <Select<ValueSource['type']>
          aria-label="引用来源类型"
          value={sourceType}
          options={SOURCE_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => onChange({
            type: 'ref',
            source: createValueSource(type, editorContext),
            pointer: '',
          })}
        />
      </InspectorField>
      {value.source.type === 'node' ? (
        <NodeReferenceFields
          source={value.source}
          pointer={value.pointer}
          nodes={editorContext?.upstreamNodes ?? []}
          onChange={onChange}
        />
      ) : null}
      {value.source.type === 'workflow_input' ? (
        <WorkflowInputReferenceFields
          source={value.source}
          pointer={value.pointer}
          inputs={editorContext?.workflowInputs ?? []}
          onChange={onChange}
        />
      ) : null}
      {value.source.type === 'variable' ? (
        <VariableReferenceFields
          source={value.source}
          pointer={value.pointer}
          variableNames={editorContext?.variableNames ?? []}
          onChange={onChange}
        />
      ) : null}
    </>
  );
}

type NodeReferenceFieldsProps = Readonly<{
  source: Extract<ValueSource, { type: 'node' }>;
  pointer: string;
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  onChange: (value: ValueExpr) => void;
}>;

function NodeReferenceFields({ source, pointer, nodes, onChange }: NodeReferenceFieldsProps) {
  const selectedNode = nodes.find((node) => node.id === source.node_id);
  const nodeOptions = nodes.map((node) => ({ value: node.id, label: node.data.label }));
  if (source.node_id && !selectedNode) {
    nodeOptions.unshift({ value: source.node_id, label: `不可用：${source.node_id}` });
  }
  const outputs = selectedNode ? getNodeValueOutputs(selectedNode.data) : [];
  const knownPointers = outputs.map((output) => ({
    value: `/${escapePointerToken(output.name)}`,
    label: output.label,
  }));
  const pointerIsKnown = pointer === ''
    || knownPointers.some((option) => option.value === pointer);
  const pointerMode = pointerIsKnown ? pointer : '__custom__';
  const pointerOptions = [
    { value: '', label: '整个输出对象' },
    ...knownPointers,
    { value: '__custom__', label: '自定义 JSON Pointer' },
  ];
  return (
    <>
      <InspectorField label="上游节点">
        <Select<string>
          aria-label="上游节点"
          value={source.node_id}
          options={nodeOptions.length > 0
            ? nodeOptions
            : [{ value: '', label: '暂无可引用的上游节点', disabled: true }]}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(nodeId) => onChange({
            type: 'ref',
            source: { type: 'node', node_id: nodeId },
            pointer: '',
          })}
        />
      </InspectorField>
      <InspectorField label="值">
        <Select<string>
          aria-label="节点输出值"
          value={pointerMode}
          options={pointerOptions}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(pointer) => onChange({
            type: 'ref',
            source,
            pointer: pointer === '__custom__' ? '/' : pointer,
          })}
        />
      </InspectorField>
      {pointerMode === '__custom__' ? (
        <InspectorField label="JSON Pointer">
          <Input
            aria-label="节点输出 JSON Pointer"
            value={pointer}
            placeholder="/path"
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => onChange({
              type: 'ref',
              source,
              pointer: event.target.value,
            })}
          />
        </InspectorField>
      ) : null}
    </>
  );
}

function WorkflowInputReferenceFields({
  source,
  pointer,
  inputs,
  onChange,
}: Readonly<{
  source: Extract<ValueSource, { type: 'workflow_input' }>;
  pointer: string;
  inputs: ReadonlyArray<WorkflowInputDefinition>;
  onChange: (value: ValueExpr) => void;
}>) {
  const options = inputs.map((input) => ({ value: input.key, label: input.key }));
  return (
    <>
      <InspectorField label="输入字段">
        <Select<string>
          aria-label="工作流输入字段"
          value={source.key}
          options={options.length > 0
            ? options
            : [{ value: '', label: '暂无工作流输入', disabled: true }]}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(key) => onChange({
            type: 'ref',
            source: { type: 'workflow_input', key },
            pointer,
          })}
        />
      </InspectorField>
      <JsonPointerField source={source} pointer={pointer} onChange={onChange} />
    </>
  );
}

function VariableReferenceFields({
  source,
  pointer,
  variableNames,
  onChange,
}: Readonly<{
  source: Extract<ValueSource, { type: 'variable' }>;
  pointer: string;
  variableNames: ReadonlyArray<string>;
  onChange: (value: ValueExpr) => void;
}>) {
  const dataListId = useId();
  return (
    <>
      <InspectorField label="变量名称">
        <Input
          aria-label="运行变量名称"
          value={source.name}
          list={dataListId}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            type: 'ref',
            source: { type: 'variable', name: event.target.value },
            pointer,
          })}
        />
      </InspectorField>
      <datalist id={dataListId}>
        {variableNames.map((name) => <option key={name} value={name} />)}
      </datalist>
      <JsonPointerField source={source} pointer={pointer} onChange={onChange} />
    </>
  );
}

/** 工作流输入与运行变量共享的任意 JSON Pointer 编辑器。 */
function JsonPointerField({
  source,
  pointer,
  onChange,
}: Readonly<{
  source: Exclude<ValueSource, { type: 'node' }>;
  pointer: string;
  onChange: (value: ValueExpr) => void;
}>) {
  return (
    <InspectorField label="JSON Pointer">
      <Input
        aria-label="引用 JSON Pointer"
        value={pointer}
        placeholder="空值表示整个值"
        containerClassName="border-slate-300 bg-white"
        onChange={(event) => onChange({
          type: 'ref',
          source,
          pointer: event.target.value,
        })}
      />
    </InspectorField>
  );
}

function ExpressionSummary({
  source,
  canOpen,
  onOpen,
}: Readonly<{ source: string; canOpen: boolean; onOpen: () => void }>) {
  return (
    <div className="rounded-md border border-slate-200 bg-white p-2.5">
      <p className="truncate font-mono text-[11px] text-slate-700">
        {source || '尚未填写表达式'}
      </p>
      <button
        type="button"
        disabled={!canOpen}
        className="mt-2 h-7 rounded-md border border-blue-200 bg-blue-50 px-2.5 text-[11px] font-medium text-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
        onClick={onOpen}
      >
        编辑表达式
      </button>
    </div>
  );
}

type LiteralFieldProps = Readonly<{
  label: string;
  value: string;
  presentation: LiteralPresentation;
  onChange: (value: string) => void;
}>;

function LiteralField({ label, value, presentation, onChange }: LiteralFieldProps) {
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

function JsonLiteralField({
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
    <InspectorField label={label}>
      <textarea
        aria-label={label}
        value={draft}
        className="h-20 w-full resize-y rounded-md border border-slate-300 bg-white px-2 py-1.5 font-mono text-[11px] leading-4 text-slate-700 outline-none focus:border-blue-400"
        onChange={(event) => {
          const nextDraft = event.target.value;
          setDraft(nextDraft);
          try {
            const parsed = JSON.parse(nextDraft) as JsonValue;
            onChange(parsed);
            setError(null);
          } catch (parseError) {
            setError(parseError instanceof Error ? parseError.message : 'JSON 格式无效');
          }
        }}
      />
      {error ? <span className="mt-1 block text-[11px] text-rose-600">{error}</span> : null}
    </InspectorField>
  );
}

/** 切换顶层模式时建立字段完整的新表达式。 */
export function createValueExpr(kind: ValueExprKind): ValueExpr {
  switch (kind) {
    case 'literal':
      return { type: kind, value: '' };
    case 'ref':
      return {
        type: kind,
        source: { type: 'node', node_id: '' },
        pointer: '',
      };
    case 'expression':
      return { type: kind, source: '' };
  }
}

function createValueSource(
  type: ValueSource['type'],
  editorContext: ValueExprEditorContextValue | null,
): ValueSource {
  switch (type) {
    case 'node':
      return { type, node_id: editorContext?.upstreamNodes[0]?.id ?? '' };
    case 'variable':
      return { type, name: editorContext?.variableNames[0] ?? '' };
    case 'workflow_input':
      return { type, key: editorContext?.workflowInputs[0]?.key ?? '' };
  }
}

function escapePointerToken(value: string): string {
  return value.replaceAll('~', '~0').replaceAll('/', '~1');
}
