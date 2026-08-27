import {
  Boxes,
  Layers3,
  PanelLeft,
  Settings,
  Workflow,
  type LucideIcon,
} from 'lucide-react';

/** 左侧工作台可切换的有限模块。 */
export type PaletteModule =
  | 'nodes'
  | 'outline'
  | 'resources'
  | 'subflows'
  | 'settings';

type PaletteModuleDefinition = Readonly<{
  /** 模块稳定键。 */
  id: PaletteModule;
  /** 用户可见名称。 */
  label: string;
  /** 占位页的功能说明。 */
  description: string;
  /** 导航图标。 */
  icon: LucideIcon;
}>;

/** 工作台模块清单同时驱动导航与占位页文案。 */
const PALETTE_MODULES = [
  { id: 'nodes', label: '节点库', description: '把节点拖到画布，组装流程。', icon: Layers3 },
  { id: 'outline', label: '流程大纲', description: '按层级查看流程，并快速定位节点。', icon: PanelLeft },
  { id: 'resources', label: '资源', description: '管理流程要用的数据和凭据。', icon: Boxes },
  { id: 'subflows', label: '子流程', description: '查看可以重复使用的流程。', icon: Workflow },
  { id: 'settings', label: '工作台设置', description: '调整节点库和编辑器偏好。', icon: Settings },
] as const satisfies ReadonlyArray<PaletteModuleDefinition>;

/** 返回指定模块的强类型定义。 */
export function findPaletteModule(moduleId: PaletteModule): PaletteModuleDefinition {
  return PALETTE_MODULES.find((module) => module.id === moduleId)
    ?? PALETTE_MODULES[0];
}

type PaletteNavigationProps = Readonly<{
  /** 当前模块。 */
  activeModule: PaletteModule;
  /** 请求切换模块。 */
  onModuleChange: (module: PaletteModule) => void;
}>;

/** 节点库底部的工作台模块导航。 */
export function PaletteNavigation({
  activeModule,
  onModuleChange,
}: PaletteNavigationProps) {
  return (
    <nav
      aria-label="工作台模块"
      className="flex h-10 shrink-0 items-center border-t border-slate-200 bg-white"
    >
      {PALETTE_MODULES.map((module) => {
        const Icon = module.icon;
        const active = module.id === activeModule;
        return (
          <button
            key={module.id}
            type="button"
            aria-label={module.label}
            aria-current={active ? 'page' : undefined}
            className={
              'relative flex h-10 flex-1 items-center justify-center ' +
              (active
                ? 'text-blue-700 after:absolute after:bottom-0 after:h-0.5 after:w-5 after:bg-blue-600'
                : 'text-slate-500 hover:bg-slate-50 hover:text-slate-900')
            }
            title={module.label}
            onClick={() => onModuleChange(module.id)}
          >
            <Icon className="size-4 shrink-0" aria-hidden="true" />
          </button>
        );
      })}
    </nav>
  );
}

/** 未接入模块的明确占位内容，避免导航按钮点击无反馈。 */
export function PaletteModulePlaceholder({
  moduleId,
}: Readonly<{ moduleId: Exclude<PaletteModule, 'nodes'> }>) {
  const module = findPaletteModule(moduleId);
  const Icon = module.icon;
  return (
    <div className="flex min-h-0 flex-1 items-start justify-center p-3 pt-6">
      <div className="w-full border border-dashed border-slate-300 bg-slate-50 px-4 py-6 text-center">
        <Icon className="mx-auto size-5 text-slate-400" aria-hidden="true" />
        <h3 className="mt-2 text-[12px] font-semibold text-slate-700">{module.label}</h3>
        <p className="mt-1 text-[11px] leading-[18px] text-slate-500">
          {module.description}
        </p>
      </div>
    </div>
  );
}
