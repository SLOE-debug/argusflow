import type { ValueExpr } from '../../../features/workflow';
import type { WorkflowSymbol } from '../../../features/workflow';

/** 以用户熟悉的来源名称展示一个结构化值引用。 */
export function ValueReferencePreview({
  value,
  symbols,
}: Readonly<{
  value: ValueExpr;
  symbols: ReadonlyArray<WorkflowSymbol>;
}>) {
  if (value.type === 'literal') return <span className="font-mono">{JSON.stringify(value.value)}</span>;
  if (value.type === 'expression') return <span className="font-mono">{value.source || '还没有填写表达式'}</span>;
  const symbol = findSymbol(value, symbols);
  if (symbol) return <span>{symbol.label}</span>;
  return (
    <span className="font-mono text-amber-700">
      {formatUnlistedReference(value)}
    </span>
  );
}

/** 保留历史 JSON Pointer 引用的可读摘要，避免选择器掩盖已有数据。 */
function formatUnlistedReference(
  value: Extract<ValueExpr, { type: 'ref' }>,
): string {
  const source = value.source.type === 'workflow_input'
    ? `输入：${value.source.key}`
    : value.source.type === 'variable'
      ? `变量：${value.source.name}`
      : `节点：${value.source.node_id}`;
  return `${source}${value.pointer || '（全部数据）'}`;
}

/** 通过稳定 source 与 pointer 将 ValueExpr 反查到编辑器符号。 */
function findSymbol(
  value: Extract<ValueExpr, { type: 'ref' }>,
  symbols: ReadonlyArray<WorkflowSymbol>,
): WorkflowSymbol | undefined {
  return symbols.find((symbol) => {
    switch (symbol.kind) {
      case 'workflow_input':
        return value.source.type === 'workflow_input'
          && value.source.key === symbol.name
          && value.pointer === '';
      case 'variable':
        return value.source.type === 'variable'
          && value.source.name === symbol.name
          && value.pointer === '';
      case 'node_output':
        return value.source.type === 'node'
          && value.source.node_id === symbol.nodeId
          && value.pointer === `/${escapePointerToken(symbol.outputName)}`;
      case 'node_result':
        return value.source.type === 'node'
          && value.source.node_id === symbol.nodeId
          && value.pointer === '';
    }
  });
}

function escapePointerToken(value: string): string {
  return value.replaceAll('~', '~0').replaceAll('/', '~1');
}
