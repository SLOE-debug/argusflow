import type { DetailsHTMLAttributes, ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { twMerge } from "tailwind-merge";
/** 原生可访问折叠面板统一箭头、键盘焦点和标题布局。 */
export function Collapse({
  title,
  extra,
  children,
  className,
  ...props
}: Omit<DetailsHTMLAttributes<HTMLDetailsElement>, "title"> & {
  readonly title: ReactNode;
  readonly extra?: ReactNode;
}) {
  return (
    <details
      {...props}
      className={twMerge("text-xs [&[open]>summary>svg]:rotate-90", className)}
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 rounded-sm py-1 outline-none focus-visible:bg-hover [&::-webkit-details-marker]:hidden">
        <ChevronRight size={12} className="shrink-0 transition-transform " />
        <span className="min-w-0 flex-1">{title}</span>
        {extra && (
          <span className="shrink-0 text-[10px] text-muted">{extra}</span>
        )}
      </summary>
      {children}
    </details>
  );
}
