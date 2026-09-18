import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { twMerge } from "tailwind-merge";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
/** 标签页共享键盘导航、滚动和可选关闭动作；业务只提供条目与选择回调。 */
export function Tabs<T extends string>({
  items,
  value,
  onChange,
  label,
  onClose,
  className,
}: {
  readonly items: readonly {
    readonly value: T;
    readonly label: ReactNode;
    readonly title: string;
    readonly marker?: string;
  }[];
  readonly value: T | null;
  readonly onChange: (value: T) => void;
  readonly label: string;
  readonly onClose?: (value: T) => void;
  readonly className?: string;
}) {
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.parentElement?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [value]);
  return (
    <div
      ref={list}
      role="tablist"
      aria-label={label}
      className={twMerge(
        "flex min-w-0 items-center gap-1 overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
        className,
      )}
      onKeyDown={(event) => {
        if (
          !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) ||
          !items.length ||
          !(event.target instanceof HTMLElement) ||
          event.target.getAttribute("role") !== "tab"
        )
          return;
        event.preventDefault();
        const index = items.findIndex((item) => item.value === value);
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? items.length - 1
              : (index + (event.key === "ArrowRight" ? 1 : -1) + items.length) %
                items.length;
        onChange(items[next].value);
        list.current
          ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
          [next]?.focus();
      }}
    >
      {items.map((item) => (
        <div
          key={item.value}
          className={
            "group flex h-7 min-w-0 max-w-48 shrink-0 items-center rounded-md " +
            (value === item.value
              ? "bg-accent-soft text-accent"
              : "text-muted hover:bg-hover")
          }
        >
          <Button
            role="tab"
            variant="ghost"
            aria-label={item.title}
            title={item.title}
            aria-selected={value === item.value}
            tabIndex={value === item.value ? 0 : -1}
            className="h-full min-w-0 flex-1 shrink justify-start overflow-hidden border-0 px-2 text-current"
            onClick={() => onChange(item.value)}
          >
            <span className="min-w-0 truncate">{item.label}</span>
            {item.marker && (
              <span
                aria-label={item.marker}
                className="size-1.5 shrink-0 rounded-full bg-accent"
              />
            )}
          </Button>
          {onClose && (
            <IconButton
              aria-label={"关闭 " + item.title}
              className="mr-1 size-5 border-0 p-0"
              onClick={() => onClose(item.value)}
            >
              <X size={11} />
            </IconButton>
          )}
        </div>
      ))}
    </div>
  );
}
