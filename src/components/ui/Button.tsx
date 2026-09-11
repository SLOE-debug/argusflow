import type { ButtonHTMLAttributes } from "react";
import { twMerge } from "tailwind-merge";

/** 工具栏和业务动作共用的紧凑按钮。 */
export function Button({
  className = "",
  variant = "default",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly variant?: "default" | "primary" | "ghost" | "danger";
}) {
  const tone =
    variant === "primary"
      ? "bg-accent text-on-accent border-transparent hover:bg-accent-hover"
      : variant === "ghost"
        ? "border-transparent hover:bg-hover"
        : variant === "danger"
          ? "border-transparent text-danger hover:bg-danger-soft"
          : "border-line bg-surface hover:bg-hover";
  return (
    <button
      type="button"
      className={twMerge(
        "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-60",
        tone,
        className,
      )}
      {...props}
    />
  );
}
