import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import type { LucideIcon } from 'lucide-react';
import type { CSSProperties, KeyboardEvent, ReactNode } from 'react';

type FlowMenuSurfaceProps = Readonly<{
  /** 当前菜单层的稳定标识，用于限制键盘焦点范围。 */
  menuId: string;
  /** 菜单的无障碍名称。 */
  ariaLabel: string;
  /** 附加定位与显示类。 */
  className?: string;
  /** 根菜单使用的屏幕定位。 */
  style?: CSSProperties;
  /** Escape 或左方向键触发的返回操作。 */
  onBack: () => void;
  /** 当前菜单内容。 */
  children: ReactNode;
}>;

type FlowMenuItemProps = Readonly<{
  /** 所属菜单层标识。 */
  menuId: string;
  /** 菜单文字。 */
  label: string;
  /** 菜单图标。 */
  icon: LucideIcon;
  /** 图标使用的强调色。 */
  iconTone?: string;
  /** 右侧快捷键提示。 */
  shortcut?: string;
  /** 当前项目是否不可操作。 */
  disabled?: boolean;
  /** 级联子菜单标识；存在时显示箭头并打开子菜单。 */
  submenuId?: string;
  /** 当前子菜单是否已展开。 */
  submenuOpen?: boolean;
  /** 打开级联子菜单。 */
  onOpenSubmenu?: () => void;
  /** 指针或键盘进入该项目时同步根菜单状态。 */
  onHighlight?: () => void;
  /** 普通菜单操作。 */
  onClick?: () => void;
  /** 级联菜单内容。 */
  children?: ReactNode;
}>;

/** Windows 风格菜单浮层的统一样式。 */
const MENU_SURFACE_CLASS_NAME = [
  'z-[120] w-48 rounded-md border border-slate-300 bg-slate-50 p-1',
  'text-xs text-slate-800 shadow-[0_8px_24px_rgba(15,23,42,.18),0_1px_3px_rgba(15,23,42,.10)]',
].join(' ');

/** 28px 高的 Windows 风格级联菜单浮层。 */
export function FlowMenuSurface({
  menuId,
  ariaLabel,
  className = '',
  style,
  onBack,
  children,
}: FlowMenuSurfaceProps) {
  /** 在当前菜单层内循环移动焦点，不进入子菜单 DOM。 */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape' || event.key === 'ArrowLeft') {
      event.preventDefault();
      event.stopPropagation();
      onBack();
      return;
    }

    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        `[data-menu-owner="${menuId}"]:not(:disabled)`,
      ),
    );
    if (items.length === 0) return;

    const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? items.length - 1
        : event.key === 'ArrowDown'
          ? (currentIndex + 1 + items.length) % items.length
          : (currentIndex - 1 + items.length) % items.length;
    items[nextIndex]?.focus();
  };

  return (
    <div
      role="menu"
      aria-label={ariaLabel}
      className={`${MENU_SURFACE_CLASS_NAME} ${className}`}
      style={style}
      onKeyDown={handleKeyDown}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {children}
    </div>
  );
}

/** 单个紧凑菜单项，并可托管一个相邻级联菜单。 */
export function FlowMenuItem({
  menuId,
  label,
  icon: Icon,
  iconTone = 'text-slate-600',
  shortcut,
  disabled = false,
  submenuId,
  submenuOpen = false,
  onOpenSubmenu,
  onHighlight,
  onClick,
  children,
}: FlowMenuItemProps) {
  /** 打开子菜单后把焦点移到其中第一项。 */
  const openAndFocusSubmenu = () => {
    if (!submenuId || !onOpenSubmenu) return;
    onOpenSubmenu();
    queueMicrotask(() => {
      document.querySelector<HTMLButtonElement>(
        `[data-menu-owner="${submenuId}"]:not(:disabled)`,
      )?.focus();
    });
  };
  const handleClick = submenuId ? openAndFocusSubmenu : onClick;
  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!submenuId || event.key !== 'ArrowRight') return;
    event.preventDefault();
    event.stopPropagation();
    openAndFocusSubmenu();
  };

  return (
    <div
      className="relative"
      onMouseEnter={onHighlight ?? onOpenSubmenu}
    >
      <button
        type="button"
        role="menuitem"
        tabIndex={-1}
        data-menu-owner={menuId}
        aria-haspopup={submenuId ? 'menu' : undefined}
        aria-expanded={submenuId ? submenuOpen : undefined}
        className={
          'flex h-7 w-full items-center gap-2 rounded-[4px] border-0 bg-transparent ' +
          'px-1.5 text-left text-xs text-slate-700 outline-none hover:bg-slate-200/80 ' +
          'focus:bg-slate-200/80 disabled:cursor-default disabled:opacity-40'
        }
        disabled={disabled}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
      >
        <Icon
          className={`size-4 shrink-0 ${iconTone}`}
          aria-hidden="true"
        />
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {shortcut ? <span className="text-[10px] text-slate-400">{shortcut}</span> : null}
        {submenuId ? <ChevronRight className="size-3.5 shrink-0 text-slate-400" aria-hidden="true" /> : null}
      </button>
      {submenuOpen ? children : null}
    </div>
  );
}

/** 菜单操作分组之间的细分隔线。 */
export function FlowMenuSeparator() {
  return <div className="mx-1 my-1 h-px bg-slate-300" role="separator" />;
}
