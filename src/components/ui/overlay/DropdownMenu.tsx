import type { LucideIcon } from 'lucide-react';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type Ref,
  type RefObject,
} from 'react';

import { Button } from '../button/Button';

export type DropdownMenuItem = Readonly<{
  /** 选项的稳定标识。 */
  id: string;
  /** 选项显示文字。 */
  label: ReactNode;
  /** 选项左侧图标。 */
  icon?: LucideIcon;
  /** 选中后执行的业务回调。 */
  onSelect: () => void;
  /** 当前选项是否不可用。 */
  disabled?: boolean;
  /** 是否使用危险操作颜色。 */
  danger?: boolean;
}>;

export type DropdownMenuTriggerProps = Readonly<{
  /** 触发器 DOM 引用。 */
  ref: Ref<HTMLButtonElement>;
  /** 触发器稳定 ID。 */
  id: string;
  /** 原生菜单语义。 */
  'aria-haspopup': 'menu';
  /** 菜单当前是否展开。 */
  'aria-expanded': boolean;
  /** 打开或关闭菜单。 */
  onClick: () => void;
  /** 触发器键盘交互。 */
  onKeyDown: (event: ReactKeyboardEvent<HTMLButtonElement>) => void;
}>;

export type DropdownMenuProps = Readonly<{
  /** 当前菜单选项。 */
  items: ReadonlyArray<DropdownMenuItem>;
  /** 受控展开状态；省略时由组件内部维护。 */
  open?: boolean;
  /** 展开状态变化回调。 */
  onOpenChange?: (open: boolean) => void;
  /** 自定义触发器渲染器。 */
  trigger?: (props: DropdownMenuTriggerProps) => ReactNode;
  /** 默认触发器文字；提供自定义 trigger 时可省略。 */
  label?: string;
  /** 菜单水平对齐方向。 */
  align?: 'start' | 'end';
  /** 外层布局附加样式。 */
  className?: string;
}>;

/** 提供 click-away、Escape、焦点移动和菜单项语义的轻量下拉菜单。 */
export function DropdownMenu({
  items,
  open: controlledOpen,
  onOpenChange,
  trigger,
  label = '更多',
  align = 'end',
  className = '',
}: DropdownMenuProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : uncontrolledOpen;
  const menuId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const setOpen = (nextOpen: boolean) => {
    if (!isControlled) setUncontrolledOpen(nextOpen);
    onOpenChange?.(nextOpen);
  };

  useEffect(() => {
    if (!open) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const firstItem = menuRef.current?.querySelector<HTMLButtonElement>(
      '[role="menuitem"]:not(:disabled)',
    );
    firstItem?.focus();
  }, [open]);

  const triggerProps: DropdownMenuTriggerProps = {
    ref: triggerRef,
    id: `${menuId}-trigger`,
    'aria-haspopup': 'menu',
    'aria-expanded': open,
    onClick: () => {
      if (items.length > 0) setOpen(!open);
    },
    onKeyDown: (event) => {
      if (event.key !== 'ArrowDown' && event.key !== 'Enter' && event.key !== ' ') return;
      event.preventDefault();
      if (items.length > 0) setOpen(true);
    },
  };

  return (
    <div
      ref={containerRef}
      className={`relative ${className}`}
    >
      {trigger ? trigger(triggerProps) : (
        <Button
          {...triggerProps}
          variant="secondary"
          size="compact"
        >
          {label}
        </Button>
      )}
      {open ? (
        <div
          ref={menuRef}
          id={menuId}
          role="menu"
          aria-labelledby={triggerProps.id}
          className={
            `absolute top-full z-50 mt-1 min-w-40 rounded-md border border-slate-200 ` +
            `bg-white p-1 shadow-lg ${align === 'end' ? 'right-0' : 'left-0'}`
          }
          onKeyDown={(event) => handleMenuKeyDown(event, menuRef)}
        >
          {items.map((item) => (
            <DropdownMenuItemButton
              key={item.id}
              item={item}
              onSelect={() => {
                if (item.disabled) return;
                setOpen(false);
                triggerRef.current?.focus();
                item.onSelect();
              }}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

type DropdownMenuItemButtonProps = Readonly<{
  /** 要渲染的菜单选项。 */
  item: DropdownMenuItem;
  /** 选中后的关闭与业务回调。 */
  onSelect: () => void;
}>;

/** 统一菜单项的图标、危险色和键盘聚焦反馈。 */
function DropdownMenuItemButton({ item, onSelect }: DropdownMenuItemButtonProps) {
  const Icon = item.icon;
  return (
    <button
      type="button"
      role="menuitem"
      disabled={item.disabled}
      className={
        'flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-[12px] outline-none ' +
        (item.danger
          ? 'text-rose-700 hover:bg-rose-50 focus-visible:bg-rose-50'
          : 'text-slate-700 hover:bg-slate-100 focus-visible:bg-slate-100') +
        ' disabled:cursor-not-allowed disabled:opacity-45'
      }
      onClick={onSelect}
    >
      {Icon ? (
        <Icon
          className="size-3.5 shrink-0"
          aria-hidden="true"
        />
      ) : null}
      <span className="truncate">{item.label}</span>
    </button>
  );
}

/** 支持上下、Home/End 在菜单项之间移动焦点。 */
function handleMenuKeyDown(
  event: ReactKeyboardEvent<HTMLDivElement>,
  menuRef: RefObject<HTMLDivElement | null>,
) {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  const items = Array.from(
    menuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [],
  );
  if (!items.length) return;
  event.preventDefault();
  const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
  const nextIndex = event.key === 'Home'
    ? 0
    : event.key === 'End'
      ? items.length - 1
      : (currentIndex + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
  items[nextIndex]?.focus();
}
