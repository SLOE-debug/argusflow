import type { ButtonHTMLAttributes } from 'react';
import type { LucideIcon } from 'lucide-react';

import { buttonStyles } from './buttonStyles';
import type { ButtonSize, ButtonVariant } from './types';

export type IconButtonProps = Readonly<
  Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    'aria-label' | 'children' | 'className' | 'disabled'
  > & {
    /** 图标按钮的可访问名称。 */
    label: string;
    /** 要展示的 Lucide 图标。 */
    icon: LucideIcon;
    /** 图标按钮语义变体。 */
    variant?: ButtonVariant;
    /** 图标按钮视觉密度。 */
    size?: ButtonSize;
    /** 覆盖图标尺寸；标题栏命令可用它恢复更醒目的图标比例。 */
    iconClassName?: string;
    /** 业务层附加布局样式。 */
    className?: string;
    /** 当前是否不可用。 */
    disabled?: boolean;
  }
>;

/** 用稳定的名称、尺寸和焦点反馈替代重复的图标按钮样式。 */
export function IconButton({
  label,
  icon: Icon,
  variant = 'ghost',
  size = 'compact',
  iconClassName = 'size-3.5',
  className = '',
  disabled = false,
  title,
  type,
  ...buttonProps
}: IconButtonProps) {
  const squareSizeClassName = size === 'compact' ? 'size-[26px]' : 'size-8';

  return (
    <button
      {...buttonProps}
      type={type ?? 'button'}
      aria-label={label}
      title={title ?? label}
      disabled={disabled}
      className={buttonStyles({
        variant,
        size,
        className: `${squareSizeClassName} p-0 ${className}`,
      })}
    >
      <Icon
        className={`${iconClassName} shrink-0`}
        aria-hidden="true"
      />
    </button>
  );
}
