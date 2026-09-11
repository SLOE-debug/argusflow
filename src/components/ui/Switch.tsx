import { Button } from "./Button";
import type { AriaAttributes } from "react";
import { isInvalid } from "./controlStyles";

/** 自绘开关，标签和滑块属于同一个可聚焦操作。 */
export function Switch({
  checked,
  onCheckedChange,
  label,
  disabled,
  ...aria
}: AriaAttributes & {
  readonly checked: boolean;
  readonly onCheckedChange: (checked: boolean) => void;
  readonly label: string;
  readonly disabled?: boolean;
}) {
  return (
    <Button
      {...aria}
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      variant="ghost"
      className="gap-2 px-0 font-normal"
      onClick={() => onCheckedChange(!checked)}
    >
      <span
        aria-hidden="true"
        className={
          "flex h-4 w-7 shrink-0 items-center rounded-full p-0.5 transition-colors " +
          (isInvalid(aria["aria-invalid"])
            ? "bg-danger"
            : checked
              ? "bg-accent"
              : "bg-strong")
        }
      >
        <span
          className={
            "size-3 rounded-full bg-white shadow-xs transition-transform " +
            (checked ? "translate-x-3" : "")
          }
        />
      </span>
      {label}
    </Button>
  );
}
