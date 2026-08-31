import Search from 'lucide-react/dist/esm/icons/search.mjs';
import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  ValueExpr,
  WorkflowSymbol,
  WorkflowSymbolRegistry,
} from '../../../features/workflow';
import { symbolToValueExpr } from '../../../features/workflow';
import { Button, Input } from '../../ui';
import { ValueReferencePreview } from './ValueReferencePreview';

type ValuePickerProps = Readonly<{
  /** 当前完整值表达式。 */
  value: ValueExpr;
  /** 从当前工作流快照派生的值目录。 */
  symbols?: WorkflowSymbolRegistry;
  /** 允许测试和轻量调用方直接传入扁平符号。 */
  symbolList?: ReadonlyArray<WorkflowSymbol>;
  /** 选中一个值后写回 ValueExpr。 */
  onChange: (value: ValueExpr) => void;
  /** 当前消费者节点的值选择器名称。 */
  ariaLabel?: string;
}>;

/** 按输入、变量和节点输出分组搜索值，并映射回现有 ValueExpr。 */
export function ValuePicker({
  value,
  symbols,
  symbolList,
  onChange,
  ariaLabel = '选择工作流值',
}: ValuePickerProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const allSymbols = symbolList ?? flattenRegistry(symbols);
  const groups = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    const filtered = allSymbols.filter((symbol) => normalized.length === 0 || [
      symbol.name,
      symbol.label,
      symbol.kind === 'node_output' || symbol.kind === 'node_result' ? symbol.nodeId : '',
      symbol.kind === 'node_output' || symbol.kind === 'node_result' ? symbol.nodeLabel : '',
    ].some((candidate) => candidate.toLocaleLowerCase().includes(normalized)));
    return [
      { key: 'workflow_input', label: '输入参数' },
      { key: 'variable', label: '工作流变量' },
      { key: 'node', label: '节点输出' },
    ].map((group) => ({
      ...group,
      symbols: filtered.filter((symbol) => group.key === 'node'
        ? symbol.kind === 'node_output' || symbol.kind === 'node_result'
        : symbol.kind === group.key),
    })).filter((group) => group.symbols.length > 0);
  }, [allSymbols, query]);

  useEffect(() => {
    if (!open) return undefined;
    /** 值菜单是轻量 popover，点击字段外部时应关闭并清空临时搜索。 */
    const handlePointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) {
        setOpen(false);
        setQuery('');
      }
    };
    document.addEventListener('pointerdown', handlePointerDown);
    return () => document.removeEventListener('pointerdown', handlePointerDown);
  }, [open]);

  return (
    <div ref={rootRef} className="relative min-w-0">
      <Button
        variant="secondary"
        className="min-h-8 h-auto w-full justify-start px-2.5 text-left font-normal"
        aria-label={ariaLabel}
        aria-expanded={open}
        aria-haspopup="listbox"
        onClick={() => setOpen((current) => !current)}
      >
        <span className="min-w-0 flex-1 truncate">
          {value.type === 'ref' ? (
            <ValueReferencePreview value={value} symbols={allSymbols} />
          ) : (
            <span className="text-slate-500">选择一个工作流值</span>
          )}
        </span>
        <span className="text-slate-400" aria-hidden="true">⌄</span>
      </Button>
      {open ? (
        <div className="absolute top-full left-0 z-40 mt-1 w-full min-w-[260px] rounded-lg border border-slate-200 bg-white p-2 shadow-[0_10px_28px_rgba(15,23,42,.14)]">
          <Input
            autoFocus
            aria-label="搜索工作流值"
            value={query}
            startAdornment={<Search className="ml-2 size-3.5" aria-hidden="true" />}
            placeholder="搜索值…"
            onChange={(event) => setQuery(event.target.value)}
          />
          <div className="mt-2 max-h-56 overflow-y-auto" role="listbox" aria-label="工作流值列表">
            {groups.length === 0 ? (
              <p className="px-2 py-3 text-[11px] text-slate-400">没有匹配的值。</p>
            ) : groups.map((group) => (
              <div key={group.key} className="mb-2 last:mb-0">
                <p className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-slate-400">{group.label}</p>
                {group.symbols.map((symbol) => (
                  <Button
                    key={symbol.id}
                    variant="ghost"
                    size="compact"
                    role="option"
                    aria-selected={isSameValue(value, symbol)}
                    disabled={!symbol.available}
                    className="h-auto w-full items-start justify-start whitespace-normal px-2 py-1.5 text-left text-[11px] font-normal text-slate-700 hover:bg-blue-50"
                    onClick={() => {
                      onChange(symbolToValueExpr(symbol));
                      setOpen(false);
                      setQuery('');
                    }}
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block truncate">{symbol.label}</span>
                      <span className="block truncate font-mono text-[10px] text-slate-400">
                        {formatSymbolPath(symbol)}
                      </span>
                    </span>
                    {!symbol.available ? (
                      <span
                        className="max-w-[120px] truncate text-[10px] text-amber-600"
                        title={symbol.unavailableReason}
                      >
                        {symbol.unavailableReason ?? '当前不可用'}
                      </span>
                    ) : null}
                  </Button>
                ))}
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

/** 展示与 Runtime 根一致的可复制值路径，不暴露内部 Symbol ID。 */
function formatSymbolPath(symbol: WorkflowSymbol): string {
  switch (symbol.kind) {
    case 'workflow_input':
      return `input[${JSON.stringify(symbol.name)}]`;
    case 'variable':
      return `vars[${JSON.stringify(symbol.name)}]`;
    case 'node_result':
      return `nodes[${JSON.stringify(symbol.nodeId)}]`;
    case 'node_output':
      return `nodes[${JSON.stringify(symbol.nodeId)}][${JSON.stringify(symbol.outputName)}]`;
  }
}

function flattenRegistry(registry: WorkflowSymbolRegistry | undefined): ReadonlyArray<WorkflowSymbol> {
  if (!registry) return [];
  return [...registry.inputs, ...registry.variables, ...registry.nodeOutputs];
}

function isSameValue(value: ValueExpr, symbol: WorkflowSymbol): boolean {
  return value.type === 'ref'
    && JSON.stringify(value) === JSON.stringify(symbolToValueExpr(symbol));
}
