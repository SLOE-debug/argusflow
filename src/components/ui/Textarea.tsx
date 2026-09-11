import type { TextareaHTMLAttributes, Ref } from "react";
import { twMerge } from "tailwind-merge";
import { CONTROL_STYLE, isInvalid } from "./controlStyles";

/** 多行文本与输入框使用一致的边框和错误状态。 */
export function Textarea({
  className,
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement> & {
  readonly ref?: Ref<HTMLTextAreaElement>;
}) {
  return (
    <div
      data-invalid={isInvalid(props["aria-invalid"])}
      className={twMerge(CONTROL_STYLE, "flex overflow-hidden", className)}
    >
      <textarea
        {...props}
        className="min-h-16 w-full min-w-0 resize-y bg-transparent px-2.5 py-2 text-inherit leading-5 outline-none placeholder:text-muted disabled:cursor-not-allowed"
      />
    </div>
  );
}
