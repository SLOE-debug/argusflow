type NodePaletteProps = {
  /** 新增 Log 或 Delay 节点的回调；Start/End 为固定节点不可新增。 */
  onAdd: (kind: 'log' | 'delay') => void;
};

/** 提供可编辑节点类型入口及当前版本的自动化能力提示。 */
export function NodePalette({ onAdd }: NodePaletteProps) {
  return (
    <aside className="argus-sidebar border-r p-5">
      <div className="mb-5">
        <p className="argus-section-label font-bold tracking-[0.16em] uppercase">节点</p>
        <h2 className="argus-title mt-1.5 font-bold">流程组件</h2>
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
      <div className="argus-callout mt-7 rounded-xl border border-dashed p-3.5">
        <p className="argus-body text-sm font-semibold">RPA 节点</p>
        <p className="mt-1.5 text-xs leading-5">
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
      className="argus-card-button flex w-full items-center gap-3 px-3.5 py-3.5 text-left"
    >
      <span className="argus-icon-tile flex h-9 w-9 items-center justify-center rounded-lg text-base font-bold">
        {icon}
      </span>
      <span>
        <span className="argus-body block text-sm font-semibold">{title}</span>
        <span className="argus-muted mt-0.5 block text-xs">{description}</span>
      </span>
    </button>
  );
}
