import Check from 'lucide-react/dist/esm/icons/check.mjs';
import ChevronDown from 'lucide-react/dist/esm/icons/chevron-down.mjs';
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentPropsWithoutRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react';

import {
  FORM_CONTROL_DENSITY_CLASS_NAMES,
  type FormControlDensity,
} from './formControlStyles';

export type SelectOption<Value extends string> = Readonly<{
  /** 提交给调用方的稳定选项值。 */
  value: Value;
  /** 向用户展示的选项文字。 */
  label: string;
  /** 仅在展开菜单中展示的辅助说明，触发器保持简洁。 */
  description?: string;
  /** 当前选项是否不可选择。 */
  disabled?: boolean;
}>;

type SelectTriggerProps = Omit<
  ComponentPropsWithoutRef<'button'>,
  | 'aria-activedescendant'
  | 'aria-controls'
  | 'aria-expanded'
  | 'aria-haspopup'
  | 'children'
  | 'className'
  | 'disabled'
  | 'type'
  | 'value'
>;

export type SelectProps<Value extends string> = Readonly<
  SelectTriggerProps & {
    /** 当前受控选项值。 */
    value: Value;
    /** 只读选项清单。 */
    options: ReadonlyArray<SelectOption<Value>>;
    /** 选项变化时返回强类型值。 */
    onValueChange?: (value: Value) => void;
    /** 控件是否不可打开和选择。 */
    disabled?: boolean;
    /** 控件视觉密度；紧凑模式适用于桌面标题栏。 */
    density?: FormControlDensity;
    /** 触发按钮的附加样式。 */
    className?: string;
    /** 选择框外层容器的附加布局与尺寸。 */
    containerClassName?: string;
    /** 选项文字之前的图标或其他纯视觉元素。 */
    startAdornment?: ReactNode;
  }
>;

type SelectOptionButtonProps<Value extends string> = Readonly<{
  /** 当前选项在 listbox 中的稳定 DOM 标识。 */
  id: string;
  /** 当前选项及其可选择状态。 */
  option: SelectOption<Value>;
  /** 当前选项是否为受控值。 */
  selected: boolean;
  /** 当前键盘高亮选项。 */
  highlighted: boolean;
  /** 选项被指针或键盘选择时的回调。 */
  onSelect: () => void;
  /** 指针移入时更新键盘高亮。 */
  onHighlight: () => void;
}>;

/** 基于 button 与 listbox 的现代化下拉控件，不依赖浏览器原生 select 样式。 */
export function Select<Value extends string>({
  value,
  options,
  onValueChange,
  disabled = false,
  density = 'standard',
  className = '',
  containerClassName = '',
  startAdornment,
  onClick,
  onKeyDown,
  onPointerDown,
  ...buttonProps
}: SelectProps<Value>) {
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listboxId = useId();
  const [open, setOpen] = useState(false);
  const selectedIndex = options.findIndex((option) => option.value === value);
  const firstEnabledIndex = findEnabledOptionIndex(options, 0, 1);
  const [highlightedIndex, setHighlightedIndex] = useState(
    selectedIndex >= 0 && !options[selectedIndex]?.disabled
      ? selectedIndex
      : firstEnabledIndex,
  );
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : undefined;
  const densityClassNames = FORM_CONTROL_DENSITY_CLASS_NAMES[density];
  const canOpen = !disabled && firstEnabledIndex >= 0;
  const activeOptionId = highlightedIndex >= 0
    ? `${listboxId}-option-${highlightedIndex}`
    : undefined;

  /** 关闭菜单后把焦点交还给触发按钮，避免键盘操作丢失位置。 */
  const closeMenu = useCallback(() => {
    setOpen(false);
    triggerRef.current?.focus();
  }, []);

  /** 只在选项存在且控件未禁用时打开自绘菜单。 */
  const openMenu = useCallback((direction: 1 | -1 = 1) => {
    if (!canOpen) return;
    setHighlightedIndex((current) => {
      if (current >= 0 && !options[current]?.disabled) return current;
      return findEnabledOptionIndex(options, direction > 0 ? 0 : options.length - 1, direction);
    });
    setOpen(true);
  }, [canOpen, options]);

  /** 选择值后保持受控状态，并关闭菜单。 */
  const selectOption = useCallback((index: number) => {
    const option = options[index];
    if (!option || option.disabled) return;
    onValueChange?.(option.value);
    closeMenu();
  }, [closeMenu, onValueChange, options]);

  /** 键盘导航只经过可选项，避免把 disabled 项暴露成可执行动作。 */
  const handleKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented) return;

    if (!open) {
      if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        openMenu(1);
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        openMenu(-1);
      }
      return;
    }

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        setHighlightedIndex((current) => findEnabledOptionIndex(
          options,
          Math.max(0, current) + 1,
          1,
        ));
        break;
      case 'ArrowUp':
        event.preventDefault();
        setHighlightedIndex((current) => findEnabledOptionIndex(
          options,
          Math.min(options.length - 1, current < 0 ? options.length - 1 : current - 1),
          -1,
        ));
        break;
      case 'Home':
        event.preventDefault();
        setHighlightedIndex(findEnabledOptionIndex(options, 0, 1));
        break;
      case 'End':
        event.preventDefault();
        setHighlightedIndex(findEnabledOptionIndex(options, options.length - 1, -1));
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        if (highlightedIndex >= 0) selectOption(highlightedIndex);
        break;
      case 'Escape':
        event.preventDefault();
        closeMenu();
        break;
      case 'Tab':
        setOpen(false);
        break;
      default:
        break;
    }
  };

  /** 菜单打开时点击控件外部即可关闭，不影响控件内部选项点击。 */
  useEffect(() => {
    if (!open) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && !rootRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    document.addEventListener('pointerdown', handlePointerDown);
    return () => document.removeEventListener('pointerdown', handlePointerDown);
  }, [open]);

  /** 外部值变化时同步高亮位置，确保菜单总是从当前值继续导航。 */
  useEffect(() => {
    if (selectedIndex >= 0 && !options[selectedIndex]?.disabled) {
      setHighlightedIndex(selectedIndex);
    }
  }, [options, selectedIndex]);

  const handleTriggerClick = (event: ReactMouseEvent<HTMLButtonElement>) => {
    onClick?.(event);
    if (event.defaultPrevented || !canOpen) return;
    if (open) closeMenu();
    else openMenu();
  };

  const handleTriggerPointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    onPointerDown?.(event);
    if (!event.defaultPrevented) event.stopPropagation();
  };

  return (
    <div
      ref={rootRef}
      className={
        'relative flex min-w-0 w-full items-center rounded-md border ' +
        'border-slate-200 bg-slate-50 text-slate-600 transition-colors ' +
        `${open ? 'border-blue-400 bg-white ring-1 ring-blue-100' : ''} ` +
        `${densityClassNames.container} ${containerClassName}`
      }
    >
      {startAdornment}
      <button
        {...buttonProps}
        ref={triggerRef}
        type="button"
        role="combobox"
        disabled={disabled}
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={listboxId}
        aria-activedescendant={open ? activeOptionId : undefined}
        className={
          'flex h-full min-w-0 flex-1 items-center gap-2 border-0 bg-transparent p-0 ' +
          'text-left text-slate-800 ' +
          'outline-none disabled:cursor-not-allowed disabled:opacity-50 ' +
          'focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-500 ' +
          `${densityClassNames.text} ${startAdornment ? 'pl-1.5' : ''} ${className}`
        }
        onClick={handleTriggerClick}
        onKeyDown={handleKeyDown}
        onPointerDown={handleTriggerPointerDown}
      >
        <span className="min-w-0 flex-1 truncate">
          {selectedOption?.label ?? '请选择'}
        </span>
        <ChevronDown
          className={`size-3 shrink-0 text-slate-500 transition-transform ${open ? 'rotate-180 text-blue-600' : ''}`}
          aria-hidden="true"
        />
      </button>
      {open ? (
        <div
          id={listboxId}
          role="listbox"
          aria-label={buttonProps['aria-label'] ?? '选项'}
          className={
            'absolute top-full left-0 z-50 mt-1.5 max-h-64 min-w-full overflow-y-auto ' +
            'rounded-lg border border-slate-200 bg-white p-1.5 shadow-[0_10px_28px_rgba(15,23,42,.14)]'
          }
        >
          {options.length > 0 ? options.map((option, index) => (
            <SelectOptionButton
              key={option.value}
              id={`${listboxId}-option-${index}`}
              option={option}
              selected={index === selectedIndex}
              highlighted={index === highlightedIndex}
              onSelect={() => selectOption(index)}
              onHighlight={() => setHighlightedIndex(index)}
            />
          )) : (
            <span className="block px-2.5 py-2 text-[11px] text-slate-400">暂无可选项</span>
          )}
        </div>
      ) : null}
    </div>
  );
}

/** 渲染自绘菜单项，并保持选中、悬停和禁用态的层级一致。 */
function SelectOptionButton<Value extends string>({
  id,
  option,
  selected,
  highlighted,
  onSelect,
  onHighlight,
}: SelectOptionButtonProps<Value>) {
  return (
    <button
      id={id}
      type="button"
      role="option"
      aria-selected={selected}
      disabled={option.disabled}
      tabIndex={-1}
      className={
        'flex w-full items-start gap-2 rounded-md border-0 bg-transparent px-2.5 py-2 ' +
        'text-left text-[12px] ' +
        'outline-none transition-colors disabled:cursor-not-allowed disabled:text-slate-300 ' +
        `${highlighted ? 'bg-blue-50 text-blue-700' : 'text-slate-700 hover:bg-slate-50'} ` +
        `${selected ? 'font-medium' : ''}`
      }
      onClick={onSelect}
      onMouseEnter={onHighlight}
      onPointerDown={(event) => event.preventDefault()}
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate">{option.label}</span>
        {option.description ? (
          <span className="mt-0.5 block truncate text-[10px] font-normal text-slate-400">
            {option.description}
          </span>
        ) : null}
      </span>
      {selected ? <Check className="mt-0.5 size-3.5 shrink-0 text-blue-600" aria-hidden="true" /> : null}
    </button>
  );
}

/** 从给定索引沿方向找到第一个未禁用选项；越界后保持在列表边界。 */
function findEnabledOptionIndex<Value extends string>(
  options: ReadonlyArray<SelectOption<Value>>,
  start: number,
  direction: 1 | -1,
): number {
  if (options.length === 0) return -1;
  const boundedStart = Math.min(options.length - 1, Math.max(0, start));
  for (let index = boundedStart; index >= 0 && index < options.length; index += direction) {
    if (!options[index]?.disabled) return index;
  }
  return -1;
}
