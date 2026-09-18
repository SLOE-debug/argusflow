import { Check } from "lucide-react";
import type { SelectOption } from "./model";
/** 单选和多选共享选项渲染；超出视口的长文本换行，避免横向滚动。 */
export function SelectOptions<T extends string>({
  options,
  selected,
  active,
  listId,
  onActive,
  onSelect,
}: {
  readonly options: readonly SelectOption<T>[];
  readonly selected: readonly T[];
  readonly active?: T;
  readonly listId: string;
  readonly onActive: (value: T) => void;
  readonly onSelect: (option: SelectOption<T>) => void;
}) {
  return (
    <>
      {options.map((option, index) => (
        <div
          key={option.value}
          id={listId + "-" + index}
          role="option"
          aria-selected={selected.includes(option.value)}
          aria-disabled={option.disabled}
          data-highlighted={option.value === active}
          title={
            typeof option.label === "string" ? option.label : option.searchText
          }
          className={
            "flex min-h-7 min-w-0 cursor-default items-center gap-2 rounded px-2 py-1 " +
            (option.disabled
              ? "text-muted opacity-60"
              : option.value === active
                ? "bg-accent-soft text-accent"
                : "hover:bg-hover")
          }
          onPointerMove={() => {
            if (!option.disabled) onActive(option.value);
          }}
          onClick={() => onSelect(option)}
        >
          <span className="min-w-0 flex-1 whitespace-normal [overflow-wrap:anywhere]">
            {option.label}
          </span>
          <span className="w-3 shrink-0">
            {selected.includes(option.value) && <Check size={13} />}
          </span>
        </div>
      ))}
      {!options.length && (
        <div className="whitespace-nowrap px-2 py-2 text-muted">
          没有匹配选项
        </div>
      )}
    </>
  );
}
