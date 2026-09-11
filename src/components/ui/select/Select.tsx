import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type AriaAttributes,
  type KeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";
import { twMerge } from "tailwind-merge";
import {
  CONTROL_HEIGHT,
  CONTROL_STYLE,
  isInvalid,
  type ControlSize,
} from "../controlStyles";
import type { SelectOption } from "./model";
import { useSelectPanel } from "./useSelectPanel";

/** 受控自绘单选下拉；焦点留在 combobox，通过 active-descendant 导航选项。 */
export function Select<T extends string>({
  value,
  options,
  onValueChange,
  disabled,
  className,
  controlSize = "default",
  placeholder = "请选择",
  name,
  id,
  title,
  ...aria
}: AriaAttributes & {
  readonly value: T;
  readonly options: readonly SelectOption<T>[];
  readonly onValueChange: (value: T) => void;
  readonly disabled?: boolean;
  readonly className?: string;
  readonly controlSize?: ControlSize;
  readonly placeholder?: string;
  readonly name?: string;
  readonly id?: string;
  readonly title?: string;
}) {
  const listId = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState<T | undefined>();
  /** 短暂缓冲用于逐字符查找选项，不改变提交值。 */
  const search = useRef({ text: "", at: 0 });
  const close = useCallback(() => setOpen(false), []);
  const enabled = options.filter((option) => !option.disabled);
  const selected = options.find((option) => option.value === value);
  const highlighted =
    enabled.find((option) => option.value === active) ??
    enabled.find((option) => option.value === value) ??
    enabled[0];
  const { layout, host } = useSelectPanel(
    open && !disabled,
    trigger,
    panel,
    options.length,
    close,
  );
  useEffect(() => {
    if (disabled) close();
  }, [disabled, close]);
  useEffect(() => {
    if (open && highlighted)
      panel.current
        ?.querySelector<HTMLElement>('[data-highlighted="true"]')
        ?.scrollIntoView?.({ block: "nearest" });
  }, [open, highlighted]);
  const show = () => {
    if (disabled) return;
    setActive(
      enabled.find((option) => option.value === value)?.value ??
        enabled[0]?.value,
    );
    setOpen(true);
  };
  const commit = (option: SelectOption<T>) => {
    if (disabled || option.disabled) return;
    close();
    onValueChange(option.value);
    trigger.current?.focus();
  };
  const keyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (disabled || event.nativeEvent.isComposing) return;
    if (event.key === "Tab") {
      close();
      return;
    }
    if (event.key === "Escape") {
      if (open) {
        event.preventDefault();
        event.stopPropagation();
        close();
      }
      return;
    }
    if (
      ["ArrowDown", "ArrowUp", "Home", "End", "Enter", " "].includes(event.key)
    ) {
      event.preventDefault();
      event.stopPropagation();
      if (!open) {
        show();
        return;
      }
      if (event.key === "Enter" || event.key === " ") {
        if (highlighted) commit(highlighted);
        return;
      }
      const index = enabled.findIndex(
        (option) => option.value === highlighted?.value,
      );
      const next =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? enabled.length - 1
            : (index + (event.key === "ArrowDown" ? 1 : -1) + enabled.length) %
              enabled.length;
      setActive(enabled[next]?.value);
      return;
    }
    if (
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      const now = Date.now();
      const text =
        (now - search.current.at < 600 ? search.current.text : "") +
        event.key.toLocaleLowerCase();
      search.current = { text, at: now };
      const match = enabled.find((option) =>
        (
          option.searchText ??
          (typeof option.label === "string" ? option.label : "")
        )
          .toLocaleLowerCase()
          .startsWith(text),
      );
      if (match) {
        event.preventDefault();
        setActive(match.value);
        setOpen(true);
      }
    }
  };
  return (
    <>
      {/* combobox 是基础控件自身的语义触发器，不是业务按钮。 */}
      <button
        {...aria}
        id={id}
        title={title}
        ref={trigger}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open && !disabled}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={
          open && highlighted
            ? listId + "-" + options.indexOf(highlighted)
            : undefined
        }
        disabled={disabled}
        data-invalid={isInvalid(aria["aria-invalid"])}
        className={twMerge(
          CONTROL_STYLE,
          CONTROL_HEIGHT[controlSize],
          "inline-flex items-center justify-between gap-2 px-2.5 text-left outline-none disabled:cursor-not-allowed disabled:bg-subtle disabled:text-muted",
          className,
        )}
        onClick={() => (open ? close() : show())}
        onKeyDown={keyDown}
      >
        <span className="min-w-0 truncate">
          {selected?.label ?? placeholder}
        </span>
        <ChevronDown
          size={12}
          className={
            "shrink-0 text-muted transition-transform " +
            (open ? "rotate-180" : "")
          }
        />
      </button>
      {name && (
        <input type="hidden" name={name} value={value} disabled={disabled} />
      )}
      {open &&
        !disabled &&
        host &&
        layout &&
        createPortal(
          <div
            id={listId}
            ref={panel}
            role="listbox"
            aria-label={aria["aria-label"]}
            aria-labelledby={aria["aria-labelledby"]}
            style={layout}
            className="fixed z-[100] overflow-x-hidden overflow-y-auto rounded-md border border-line bg-surface p-1 text-xs text-ink shadow-lg"
            onPointerDown={(event) => event.preventDefault()}
          >
            {options.map((option, index) => (
              <div
                key={option.value}
                id={listId + "-" + index}
                role="option"
                aria-selected={option.value === value}
                aria-disabled={option.disabled}
                data-highlighted={option.value === highlighted?.value}
                className={
                  "flex min-h-7 cursor-default items-center gap-2 rounded px-2 py-1 " +
                  (option.disabled
                    ? "text-muted opacity-60"
                    : option.value === highlighted?.value
                      ? "bg-accent-soft text-accent"
                      : "hover:bg-hover")
                }
                onPointerMove={() => {
                  if (!option.disabled) setActive(option.value);
                }}
                onClick={() => commit(option)}
              >
                <span className="min-w-0 flex-1 break-words">
                  {option.label}
                </span>
                {option.value === value && (
                  <Check size={13} className="shrink-0" />
                )}
              </div>
            ))}
            {!options.length && (
              <div className="px-2 py-2 text-muted">没有可选项</div>
            )}
          </div>,
          host,
        )}
    </>
  );
}
