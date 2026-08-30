import LoaderCircle from 'lucide-react/dist/esm/icons/loader-circle.mjs';
import type { LucideIcon } from 'lucide-react';
import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from 'react';

import { buttonStyles } from './buttonStyles';
import type { ButtonSize, ButtonVariant } from './types';

export type ButtonProps = Readonly<
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children' | 'className' | 'disabled'> & {
    /** 按钮语义变体。 */
    variant?: ButtonVariant;
    /** 按钮视觉密度。 */
    size?: ButtonSize;
    /** 是否展示加载态并暂时阻止重复操作。 */
    loading?: boolean;
    /** 加载态替代按钮正文的可访问文字。 */
    loadingLabel?: string;
    /** 按钮正文左侧的 Lucide 图标。 */
    icon?: LucideIcon;
    /** 按钮正文。 */
    children?: ReactNode;
    /** 业务层布局附加样式。 */
    className?: string;
    /** 当前是否不可用。 */
    disabled?: boolean;
  }
>;

/** 统一按钮颜色、密度、焦点、禁用和加载状态的基础控件。 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button({
  variant = 'secondary',
  size = 'standard',
  loading = false,
  loadingLabel = '处理中…',
  icon: Icon,
  children,
  className = '',
  disabled = false,
  type,
  ...buttonProps
}, ref) {
  const isDisabled = disabled || loading;

  return (
    <button
      {...buttonProps}
      ref={ref}
      type={type ?? 'button'}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      className={buttonStyles({ variant, size, className })}
    >
      {loading ? (
        <LoaderCircle
          className="size-3.5 shrink-0 animate-spin"
          aria-hidden="true"
        />
      ) : Icon ? (
        <Icon
          className="size-3.5 shrink-0"
          aria-hidden="true"
        />
      ) : null}
      {loading ? loadingLabel : children}
    </button>
  );
});
