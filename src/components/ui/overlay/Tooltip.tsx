import { useId, useState, type ReactNode } from 'react';

export type TooltipProps = Readonly<{
  /** 悬停或聚焦时展示的说明。 */
  content: ReactNode;
  /** 需要被说明的控件。 */
  children: ReactNode;
  /** 气泡相对触发器的位置。 */
  side?: 'top' | 'bottom' | 'left' | 'right';
}>;

/** 为图标按钮等紧凑控件提供 hover/focus 可访问提示。 */
export function Tooltip({ content, children, side = 'top' }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const tooltipId = useId();
  const positionClassName = TOOLTIP_POSITION_CLASS_NAMES[side];

  return (
    <span
      className="relative inline-flex"
      aria-describedby={visible ? tooltipId : undefined}
      onBlur={() => setVisible(false)}
      onFocus={() => setVisible(true)}
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={() => setVisible(false)}
    >
      {children}
      {visible ? (
        <span
          id={tooltipId}
          role="tooltip"
          className={`pointer-events-none absolute z-50 whitespace-nowrap rounded bg-slate-900 px-2 py-1 text-[11px] text-white shadow-lg ${positionClassName}`}
        >
          {content}
        </span>
      ) : null}
    </span>
  );
}

/** Tooltip 四个方向的最小定位样式。 */
const TOOLTIP_POSITION_CLASS_NAMES: Readonly<Record<NonNullable<TooltipProps['side']>, string>> = {
  top: 'bottom-full left-1/2 mb-1 -translate-x-1/2',
  bottom: 'left-1/2 top-full mt-1 -translate-x-1/2',
  left: 'right-full top-1/2 mr-1 -translate-y-1/2',
  right: 'left-full top-1/2 ml-1 -translate-y-1/2',
};

