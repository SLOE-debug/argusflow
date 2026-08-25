import type { Hover } from '../language/types';

const HOVER_LABELS: Readonly<Record<string, string>> = {
  'aql.hover.role': '元素语义角色',
  'aql.hover.function': 'AQL 查询函数',
  'aql.hover.property': '跨后端属性',
  'aql.hover.backend_property': '后端专用属性；兼容性由 Planner Explain 提供',
  'aql.hover.operator': 'AQL 比较运算符',
  'aql.hover.literal': 'AQL 字面量',
};

/** 当前 caret token 的轻量 Hover。 */
export function HoverPopup({ hover }: Readonly<{ hover: Hover | null }>) {
  if (!hover) {
    return null;
  }
  return (
    <div className="absolute right-2 top-2 z-30 max-w-60 rounded border border-slate-200 bg-white/95 px-2 py-1.5 shadow-sm">
      <code className="block font-mono text-[10px] text-slate-700">{hover.symbol}</code>
      <span className="mt-0.5 block text-[9px] text-slate-500">
        {HOVER_LABELS[hover.description_code] ?? hover.description_code}
      </span>
    </div>
  );
}
