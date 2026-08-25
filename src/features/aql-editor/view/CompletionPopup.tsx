import type { CompletionItem } from '../language/types';

type CompletionPopupProps = Readonly<{
  items: readonly CompletionItem[];
  onApply: (item: CompletionItem) => void;
  onClose: () => void;
}>;

/** 自研补全列表；候选全部来自 Rust WASM language service。 */
export function CompletionPopup({ items, onApply, onClose }: CompletionPopupProps) {
  if (items.length === 0) {
    return null;
  }

  return (
    <div className="absolute bottom-2 left-12 z-40 max-h-44 w-64 overflow-auto rounded-md border border-slate-200 bg-white p-1 shadow-lg">
      <div className="flex items-center justify-between px-1.5 py-1 text-[9px] text-slate-400">
        <span>AQL 补全</span>
        <button type="button" className="hover:text-slate-700" onClick={onClose}>关闭</button>
      </div>
      {items.map((item) => (
        <button
          key={`${item.kind}-${item.label}`}
          type="button"
          className="flex w-full items-center justify-between gap-2 rounded px-1.5 py-1 text-left hover:bg-blue-50"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => onApply(item)}
        >
          <span className="font-mono text-[10px] text-slate-700">{item.label}</span>
          <span className="truncate text-[9px] text-slate-400">{item.detail}</span>
        </button>
      ))}
    </div>
  );
}
