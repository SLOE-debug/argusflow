import type { AriaAttributes } from "react";
import { Check, Minus } from "lucide-react";
import { Button } from "./Button";
import { isInvalid } from "./controlStyles";

/** 自绘复选框，受控状态通过值回调提交，Space/Enter 沿用按钮键盘语义。 */
export function Checkbox({
  checked,
  onCheckedChange,
  label,
  disabled,
  indeterminate = false,
  ...aria
}: AriaAttributes & {
  readonly checked: boolean;
  readonly onCheckedChange: (checked: boolean) => void;
  readonly label: string;
  readonly disabled?: boolean;
  readonly indeterminate?: boolean;
}) {
  const selected = checked || indeterminate;
  return (
    <Button
      {...aria}
      variant="ghost"
      role="checkbox"
      aria-checked={indeterminate ? "mixed" : checked}
      disabled={disabled}
      className="justify-start gap-2 px-0 font-normal"
      onClick={() => onCheckedChange(indeterminate || !checked)}
    >
      <span
        aria-hidden="true"
        className={
          "flex size-4 shrink-0 items-center justify-center rounded border " +
          (isInvalid(aria["aria-invalid"])
            ? "border-danger bg-danger-soft text-danger"
            : selected
              ? "border-accent bg-accent text-on-accent"
              : "border-strong bg-surface")
        }
      >
        {indeterminate ? (
          <Minus size={12} strokeWidth={3} />
        ) : (
          checked && <Check size={12} strokeWidth={3} />
        )}
      </span>
      {label}
    </Button>
  );
}
