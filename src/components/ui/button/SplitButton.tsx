import { ChevronDown, type LucideIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';

import {
  DropdownMenu,
  type DropdownMenuItem,
  type DropdownMenuTriggerProps,
} from '../overlay/DropdownMenu';
import { Button } from './Button';
import { buttonStyles } from './buttonStyles';
import type { ButtonSize, ButtonVariant } from './types';

export type SplitButtonProps = Readonly<{
  /** 主操作显示文字。 */
  label: ReactNode;
  /** 主操作的 Lucide 图标。 */
  icon?: LucideIcon;
  /** 主操作回调。 */
  onPrimaryClick: () => void;
  /** 右侧菜单选项；为空时仍保留菜单入口的视觉占位。 */
  menu?: ReadonlyArray<DropdownMenuItem>;
  /** 按钮语义变体。 */
  variant?: ButtonVariant;
  /** 按钮视觉密度。 */
  size?: ButtonSize;
  /** 当前是否不可用。 */
  disabled?: boolean;
  /** 是否展示主操作加载态。 */
  loading?: boolean;
  /** 加载态替代主操作文字的可访问文字。 */
  loadingLabel?: string;
  /** 外层布局附加样式。 */
  className?: string;
}>;

/** 将主操作与后续选项入口组合为统一的紧凑 Split Button。 */
export function SplitButton({
  label,
  icon,
  onPrimaryClick,
  menu = [],
  variant = 'primary',
  size = 'compact',
  disabled = false,
  loading = false,
  loadingLabel = '处理中…',
  className = '',
}: SplitButtonProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuButtonLabel = typeof label === 'string' ? `${label}选项` : '更多选项';

  return (
    <div className={`inline-flex shrink-0 overflow-visible rounded-md ${className}`}>
      <Button
        variant={variant}
        size={size}
        icon={icon}
        loading={loading}
        loadingLabel={loadingLabel}
        disabled={disabled}
        className="rounded-r-none border-r-0"
        onClick={onPrimaryClick}
      >
        {label}
      </Button>
      <DropdownMenu
        items={menu}
        open={menuOpen}
        onOpenChange={setMenuOpen}
        trigger={(triggerProps) => (
          <SplitMenuButton
            {...triggerProps}
            label={menuButtonLabel}
            variant={variant}
            size={size}
            disabled={disabled || loading}
          />
        )}
      />
    </div>
  );
}

type SplitMenuButtonProps = DropdownMenuTriggerProps & Readonly<{
  /** 菜单入口的可访问名称。 */
  label: string;
  /** 菜单入口与主按钮保持一致的语义色。 */
  variant: ButtonVariant;
  /** 菜单入口与主按钮保持一致的密度。 */
  size: ButtonSize;
  /** 主操作不可用时同步禁用菜单入口。 */
  disabled?: boolean;
}>;

/** Split Button 右侧的可访问菜单入口。 */
function SplitMenuButton({
  label,
  variant,
  size,
  disabled,
  ...triggerProps
}: SplitMenuButtonProps) {
  const menuSizeClassName = size === 'compact' ? 'h-[26px] w-[26px]' : 'h-8 w-8';
  const menuVariantClassName = SPLIT_MENU_VARIANT_CLASS_NAMES[variant];

  return (
    <button
      {...triggerProps}
      type="button"
      aria-label={label}
      disabled={disabled}
      className={buttonStyles({
        variant,
        size,
        className: `${menuSizeClassName} rounded-l-none border-l ${menuVariantClassName} p-0`,
      })}
    >
      <ChevronDown
        className="size-3 shrink-0"
        aria-hidden="true"
      />
    </button>
  );
}

/** Split Button 菜单入口的分隔边界和悬停颜色。 */
const SPLIT_MENU_VARIANT_CLASS_NAMES: Readonly<Record<ButtonVariant, string>> = {
  primary: 'border-l-blue-500 hover:border-l-blue-400',
  secondary: 'border-l-slate-200 hover:border-l-slate-300',
  ghost: 'border-l-transparent',
  danger: 'border-l-rose-200 hover:border-l-rose-300',
};
