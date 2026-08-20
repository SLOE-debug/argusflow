type NodePaletteProps = {
  /** 新增 Log 或 Delay 节点的回调；Start/End 为固定节点不可新增。 */
  onAdd: (kind: 'log' | 'delay') => void;
};

/** 提供可编辑节点类型入口及当前版本的自动化能力提示。 */
export function NodePalette({ onAdd }: NodePaletteProps) {
  return (
    <aside className="border-r border-[#1d3048] bg-[#0c1828] p-4">
      <div className="mb-4">
        <p className="text-[11px] font-semibold tracking-[0.2em] text-sky-400 uppercase">节点</p>
        <h2 className="mt-1 text-sm font-semibold text-slate-100">流程组件</h2>
      </div>
      <div className="space-y-2">
        <PaletteButton
          icon="≡"
          title="Log"
          description="输出运行信息"
          onClick={() => onAdd('log')}
        />
        <PaletteButton
          icon="◷"
          title="Delay"
          description="异步等待一段时间"
          onClick={() => onAdd('delay')}
        />
      </div>
      <div className="mt-6 rounded-xl border border-dashed border-[#29415e] bg-[#101f33]/60 p-3">
        <p className="text-xs font-medium text-slate-300">RPA 节点</p>
        <p className="mt-1 text-[11px] leading-5 text-slate-500">
          UIA、CDP 与视觉动作契约已就位，真实执行将在后续版本接入。
        </p>
      </div>
    </aside>
  );
}

type PaletteButtonProps = {
  /** 按钮左侧展示的节点图标。 */
  icon: string;
  /** 节点类型名称。 */
  title: string;
  /** 节点行为的简短说明。 */
  description: string;
  /** 点击后创建对应节点。 */
  onClick: () => void;
};

/** 节点面板中的统一样式按钮。 */
function PaletteButton({ icon, title, description, onClick }: PaletteButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-3 rounded-xl border border-[#253b55] bg-[#122238] px-3 py-3 text-left transition hover:border-sky-500/60 hover:bg-[#172c46]"
    >
      <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-sky-400/10 text-sky-300">
        {icon}
      </span>
      <span>
        <span className="block text-sm font-medium text-slate-200">{title}</span>
        <span className="block text-[11px] text-slate-500">{description}</span>
      </span>
    </button>
  );
}
