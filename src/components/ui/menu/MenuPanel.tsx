import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { ChevronRight } from "lucide-react";
import { Button } from "../Button";
import type { MenuAnchor, MenuItem } from "./model";
import { useMenuPosition } from "./useMenuPosition";

/** 每层独立管理键盘焦点和当前展开项，滚动区不裁切级联浮层。 */
export function MenuPanel({
  items,
  label,
  anchor,
  focusOnOpen,
  onBack,
  onClose,
}: {
  readonly items: readonly MenuItem[];
  readonly label: string;
  readonly anchor: MenuAnchor;
  readonly focusOnOpen: boolean;
  readonly onBack: () => void;
  readonly onClose: () => void;
}) {
  const id = useId();
  const ref = useRef<HTMLDivElement>(null);
  const position = useMenuPosition(ref, anchor);
  /** 保存触发元素，使子菜单返回和重新定位使用同一锚点。 */
  const [open, setOpen] = useState<{
    readonly index: number;
    readonly trigger: HTMLButtonElement;
    readonly focus: boolean;
  } | null>(null);
  const submenu = open ? items[open.index] : undefined;
  useLayoutEffect(() => {
    if (focusOnOpen) {
      const first = ref.current?.querySelector<HTMLButtonElement>(
        "button:not(:disabled)",
      );
      (first ?? ref.current)?.focus();
    }
  }, [focusOnOpen]);
  const back = () => {
    const trigger = open?.trigger;
    setOpen(null);
    trigger?.focus();
  };
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    if (["Escape", "ArrowLeft", "Tab"].includes(event.key)) {
      event.preventDefault();
      if (event.key === "Tab") onClose();
      else onBack();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const buttons = [
      ...event.currentTarget.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    ];
    if (!buttons.length) return;
    const index = buttons.findIndex(
      (button) => button === document.activeElement,
    );
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
            buttons.length;
    buttons[next]?.focus();
  };
  return (
    <>
      <div
        ref={ref}
        id={id}
        role="menu"
        aria-label={label}
        tabIndex={-1}
        style={position}
        className="fixed z-[120] max-h-[calc(100vh-16px)] w-48 max-w-[calc(100vw-16px)] overflow-y-auto rounded-md border border-strong/70 bg-surface p-1 text-xs text-ink shadow-lg outline-none"
        onKeyDown={keyDown}
        onScroll={() => setOpen(null)}
      >
        {items.map((item, index) =>
          item.type === "separator" ? (
            <div
              key={item.id}
              role="separator"
              className="mx-1 my-1 h-px bg-line"
            />
          ) : (
            <Button
              key={item.label}
              role="menuitem"
              tabIndex={-1}
              variant="ghost"
              disabled={item.disabled}
              title={item.disabled ? item.reason : undefined}
              aria-haspopup={item.type === "submenu" ? "menu" : undefined}
              aria-expanded={
                item.type === "submenu" ? open?.index === index : undefined
              }
              className={
                "h-7 w-full justify-start gap-2 rounded-sm border-0 px-1.5 text-[11px] font-normal focus:bg-hover focus-visible:outline-none disabled:cursor-default disabled:opacity-40 " +
                (open?.index === index ? "bg-hover " : "") +
                (item.type === "action" && item.danger
                  ? "text-danger hover:bg-danger-soft focus:bg-danger-soft"
                  : "")
              }
              onMouseEnter={(event) => {
                if (!item.disabled && item.type === "submenu")
                  setOpen({
                    index,
                    trigger: event.currentTarget,
                    focus: false,
                  });
                else setOpen(null);
              }}
              onFocus={() => {
                if (open && open.index !== index) setOpen(null);
              }}
              onKeyDown={(event) => {
                if (event.key === "ArrowRight" && item.type === "submenu") {
                  event.preventDefault();
                  event.stopPropagation();
                  setOpen({ index, trigger: event.currentTarget, focus: true });
                }
              }}
              onClick={(event) => {
                if (item.type === "submenu")
                  setOpen({ index, trigger: event.currentTarget, focus: true });
                else {
                  onClose();
                  item.action();
                }
              }}
            >
              <span
                aria-hidden="true"
                className="flex size-4 shrink-0 items-center justify-center [&>svg]:size-3.5"
              >
                {item.icon}
              </span>
              <span className="min-w-0 flex-1 truncate text-left">
                {item.label}
              </span>
              {item.type === "submenu" ? (
                <ChevronRight
                  size={12}
                  className="shrink-0 text-muted"
                  aria-hidden="true"
                />
              ) : (
                item.shortcut && (
                  <kbd className="shrink-0 font-sans text-[10px] text-muted">
                    {item.shortcut}
                  </kbd>
                )
              )}
            </Button>
          ),
        )}
      </div>
      {open && submenu?.type === "submenu" && !submenu.disabled && (
        <MenuPanel
          key={submenu.label}
          items={submenu.items}
          label={submenu.label}
          anchor={{ type: "submenu", trigger: open.trigger }}
          focusOnOpen={open.focus}
          onBack={back}
          onClose={onClose}
        />
      )}
    </>
  );
}
