import type { ComponentProps } from "react";
import { twMerge } from "tailwind-merge";
import { Button } from "./Button";
/** 轻量文本动作统一使用主题蓝色与下划线，保留按钮语义和禁用状态。 */
export function LinkButton({
  className,
  ...props
}: Omit<ComponentProps<typeof Button>, "variant">) {
  return (
    <Button
      {...props}
      variant="ghost"
      className={twMerge(
        "h-auto rounded-sm border-0 px-0 py-1 font-normal text-accent underline decoration-accent/60 underline-offset-4 hover:bg-transparent hover:text-accent-hover focus-visible:bg-accent-soft disabled:no-underline",
        className,
      )}
    />
  );
}
