import type { ButtonHTMLAttributes, Ref } from "react";
import { twMerge } from "tailwind-merge";

/** 工具栏和业务动作共用的紧凑按钮。 */
export function Button({
  className = "",
  variant = "default",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly ref?: Ref<HTMLButtonElement>;
  readonly variant?: "default" | "primary" | "ghost" | "danger";
}) {
  const tone =
    variant === "primary"
      ? "bg-accent text-on-accent border-transparent hover:bg-accent-hover focus-visible:bg-accent-hover"
      : variant === "ghost"
        ? "border-transparent hover:bg-hover focus-visible:bg-hover"
        : variant === "danger"
          ? "border-transparent bg-danger text-on-accent hover:bg-danger/90 focus-visible:bg-danger/85"
          : "border-line bg-surface hover:bg-hover focus-visible:bg-hover focus-visible:border-strong";
  return (
    <button
      type="button"
      className={twMerge(
        "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md border px-2.5 text-xs font-medium outline-none transition-colors disabled:cursor-not-allowed disabled:opacity-60",
        tone,
        className,
      )}
      {...props}
    />
  );
}
