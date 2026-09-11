import type { InputHTMLAttributes, ReactNode, Ref } from "react";
import { twMerge } from "tailwind-merge";
import {
  CONTROL_HEIGHT,
  CONTROL_STYLE,
  isInvalid,
  type ControlSize,
} from "./controlStyles";

/** 输入外壳负责视觉，原生 input 保留输入法、选区和数值输入语义。 */
export function Input({
  className,
  controlSize = "default",
  leading,
  trailing,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & {
  readonly ref?: Ref<HTMLInputElement>;
  readonly controlSize?: ControlSize;
  readonly leading?: ReactNode;
  readonly trailing?: ReactNode;
}) {
  return (
    <div
      data-invalid={isInvalid(props["aria-invalid"])}
      className={twMerge(
        CONTROL_STYLE,
        CONTROL_HEIGHT[controlSize],
        "inline-flex items-center gap-2 px-2.5 align-middle",
        className,
      )}
    >
      {leading && <span className="shrink-0 text-muted">{leading}</span>}
      <input
        {...props}
        className="h-full w-full min-w-0 flex-1 border-0 bg-transparent p-0 text-inherit outline-none placeholder:text-muted disabled:cursor-not-allowed"
      />
      {trailing}
    </div>
  );
}
