import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { ChevronDown } from "lucide-react";
import { Tag } from "../Tag";
import { useSelectPanel } from "./useSelectPanel";
import { SelectOptions } from "./SelectOptions";
import { SELECT_PANEL_STYLE } from "./panelStyle";
import type { SelectOption } from "./model";
/** 单一输入外壳内组合标签和搜索；选中后菜单保持打开，支持键盘切换和撤销标签。 */
export function TagSelect({
  values,
  options,
  onChange,
  label,
  limit,
  disabled,
}: {
  readonly values: readonly string[];
  readonly options: readonly SelectOption<string>[];
  readonly onChange: (values: readonly string[]) => void;
  readonly label: string;
  readonly limit?: number;
  readonly disabled?: boolean;
}) {
  const id = useId(),
    trigger = useRef<HTMLDivElement>(null),
    input = useRef<HTMLInputElement>(null),
    panel = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false),
    [search, setSearch] = useState(""),
    [active, setActive] = useState<string>();
  const close = useCallback(() => {
    setOpen(false);
    setSearch("");
  }, []);
  const matches = options.filter((option) =>
    (
      option.searchText ??
      (typeof option.label === "string" ? option.label : option.value)
    )
      .toLocaleLowerCase()
      .includes(search.toLocaleLowerCase()),
  );
  const displayed = matches.map((option) => ({
    ...option,
    disabled:
      option.disabled ||
      (limit !== undefined &&
        values.length >= limit &&
        !values.includes(option.value)),
  }));
  const enabled = displayed.filter((option) => !option.disabled);
  const highlighted =
    enabled.find((option) => option.value === active) ?? enabled[0];
  const { layout, host } = useSelectPanel(
    open && !disabled,
    trigger,
    panel,
    displayed.length,
    close,
  );
  useEffect(() => {
    if (disabled) close();
  }, [disabled, close]);
  useEffect(() => {
    panel.current
      ?.querySelector('[data-highlighted="true"]')
      ?.scrollIntoView?.({ block: "nearest" });
  }, [highlighted?.value]);
  const toggle = (option: SelectOption<string>) => {
    if (disabled || input.current?.matches(":disabled") || option.disabled)
      return;
    onChange(
      values.includes(option.value)
        ? values.filter((value) => value !== option.value)
        : [...values, option.value],
    );
    setSearch("");
    input.current?.focus();
  };
  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Tab") {
      close();
      return;
    }
    if (event.key === "Escape" && open) {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === "Backspace" && !search && values.length) {
      event.preventDefault();
      onChange(values.slice(0, -1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      if (open && highlighted) toggle(highlighted);
      else setOpen(true);
      return;
    }
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      if (!open && (event.key === "Home" || event.key === "End")) return;
      event.preventDefault();
      event.stopPropagation();
      const index = enabled.findIndex(
        (option) => option.value === highlighted?.value,
      );
      const next =
        !open || event.key === "Home"
          ? 0
          : event.key === "End"
            ? enabled.length - 1
            : (index + (event.key === "ArrowDown" ? 1 : -1) + enabled.length) %
              enabled.length;
      setActive(enabled[next]?.value);
      setOpen(true);
    }
  };
  return (
    <>
      <div
        ref={trigger}
        className="flex min-h-8 min-w-0 items-center gap-1 rounded-md border border-line bg-surface px-2 py-1 text-xs hover:border-strong focus-within:border-accent/60 has-[:disabled]:bg-subtle"
        onClick={() => {
          if (!disabled && !input.current?.matches(":disabled")) {
            input.current?.focus();
            setOpen(true);
          }
        }}
      >
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
          {values.map((value) => (
            <Tag
              key={value}
              disabled={disabled}
              removeLabel={"移除 " + value}
              onRemove={() => onChange(values.filter((item) => item !== value))}
            >
              {options.find((option) => option.value === value)?.label ?? value}
            </Tag>
          ))}
          {/* 搜索输入属于 UI 基础控件内部，避免嵌套另一个带边框的 Input 外壳。 */}
          <input
            ref={input}
            role="combobox"
            aria-label={label}
            aria-autocomplete="list"
            aria-expanded={open && !disabled}
            aria-controls={open ? id : undefined}
            aria-activedescendant={
              open && highlighted
                ? id + "-" + displayed.indexOf(highlighted)
                : undefined
            }
            disabled={disabled}
            value={search}
            placeholder={values.length ? "" : "搜索并选择"}
            className="h-6 min-w-8 flex-1 basis-12 bg-transparent text-xs text-ink outline-none placeholder:text-muted"
            onFocus={() => setOpen(true)}
            onChange={(event) => {
              setSearch(event.target.value);
              setActive(undefined);
              setOpen(true);
            }}
            onKeyDown={keyDown}
          />
        </div>
        <ChevronDown
          size={12}
          className={"shrink-0 text-muted " + (open ? "rotate-180" : "")}
        />
      </div>
      {open &&
        !disabled &&
        host &&
        layout &&
        createPortal(
          <div
            ref={panel}
            id={id}
            role="listbox"
            aria-label={label}
            aria-multiselectable="true"
            style={{
              ...layout,
              width: "max-content",
              minWidth: layout.width,
              maxWidth: "calc(100vw - 16px)",
            }}
            className={SELECT_PANEL_STYLE}
            onPointerDown={(event) => event.preventDefault()}
          >
            <SelectOptions
              options={displayed}
              selected={values}
              active={highlighted?.value}
              listId={id}
              onActive={setActive}
              onSelect={toggle}
            />
          </div>,
          host,
        )}
    </>
  );
}
