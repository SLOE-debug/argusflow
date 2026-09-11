import type { ComponentProps } from "react";
import { twMerge } from "tailwind-merge";
import { Button } from "./Button";

/** 图标按钮强制提供可访问名称，默认紧凑规格。 */
export function IconButton({
  className,
  ...props
}: ComponentProps<typeof Button> & { readonly "aria-label": string }) {
  return (
    <Button
      variant="ghost"
      title={props["aria-label"]}
      {...props}
      className={twMerge("size-7 shrink-0 p-0", className)}
    />
  );
}
