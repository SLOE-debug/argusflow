import { useRef, type AriaAttributes } from "react";
import { Button } from "./Button";
import { isInvalid } from "./controlStyles";

/** 单选项契约；值属于调用方的有限集合。 */
export interface RadioOption<T extends string> {
  /** 组内唯一的选项值。 */
  readonly value: T;
  /** 可访问名称和显示标签。 */
  readonly label: string;
  /** 方向键导航会跳过禁用项。 */
  readonly disabled?: boolean;
}
/** 自绘单选组，单一 Tab 入口，方向键跳过禁用项并选择。 */
export function RadioGroup<T extends string>({
  value,
  options,
  onValueChange,
  label,
  disabled,
  ...aria
}: AriaAttributes & {
  readonly value: T;
  readonly options: readonly RadioOption<T>[];
  readonly onValueChange: (value: T) => void;
  readonly label: string;
  readonly disabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const enabled = options.filter((option) => !option.disabled);
  const entry = enabled.find((option) => option.value === value) ?? enabled[0];
  return (
    <div
      {...aria}
      ref={ref}
      role="radiogroup"
      aria-label={label}
      aria-disabled={disabled}
      className="flex flex-wrap gap-x-4 gap-y-1"
      onKeyDown={(event) => {
        if (
          disabled ||
          !enabled.length ||
          ![
            "ArrowLeft",
            "ArrowRight",
            "ArrowUp",
            "ArrowDown",
            "Home",
            "End",
          ].includes(event.key)
        )
          return;
        event.preventDefault();
        event.stopPropagation();
        const index = enabled.findIndex((option) => option.value === value);
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? enabled.length - 1
              : (Math.max(0, index) +
                  (["ArrowLeft", "ArrowUp"].includes(event.key) ? -1 : 1) +
                  enabled.length) %
                enabled.length;
        onValueChange(enabled[next].value);
        ref.current
          ?.querySelectorAll<HTMLButtonElement>('[role="radio"]:not(:disabled)')
          [next]?.focus();
      }}
    >
      {options.map((option) => (
        <Button
          key={option.value}
          role="radio"
          variant="ghost"
          aria-checked={value === option.value}
          disabled={disabled || option.disabled}
          tabIndex={!disabled && option.value === entry?.value ? 0 : -1}
          className="justify-start gap-2 px-0 font-normal"
          onClick={() => onValueChange(option.value)}
        >
          <span
            aria-hidden="true"
            className={
              "flex size-4 items-center justify-center rounded-full border " +
              (isInvalid(aria["aria-invalid"])
                ? "border-danger bg-danger-soft"
                : value === option.value
                  ? "border-accent bg-accent"
                  : "border-strong bg-surface")
            }
          >
            {value === option.value && (
              <span
                className={
                  "size-1.5 rounded-full " +
                  (isInvalid(aria["aria-invalid"])
                    ? "bg-danger"
                    : "bg-on-accent")
                }
              />
            )}
          </span>
          {option.label}
        </Button>
      ))}
    </div>
  );
}
