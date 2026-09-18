import type { ComponentProps } from "react";
import { twMerge } from "tailwind-merge";
import { Button } from "./Button";
/** 文字动作仅在悬停时显示下划线，键盘焦点使用柔和底色。 */
export function LinkButton({
  className,
  ...props
}: Omit<ComponentProps<typeof Button>, "variant">) {
  return (
    <Button
      {...props}
      variant="ghost"
      className={twMerge(
        "h-auto cursor-pointer rounded-sm border-0 px-0 py-1 font-normal text-accent no-underline decoration-accent/60 underline-offset-4 hover:bg-transparent hover:text-accent-hover hover:underline focus-visible:bg-accent-soft disabled:no-underline",
        className,
      )}
    />
  );
}
