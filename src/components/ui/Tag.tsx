import { X } from "lucide-react";
import type { ReactNode } from "react";
import { IconButton } from "./IconButton";
/** 可关闭标签只维护展示；选中状态和删除语义由所属控件提供。 */
export function Tag({
  children,
  onRemove,
  removeLabel,
  disabled,
}: {
  readonly children: ReactNode;
  readonly onRemove?: () => void;
  readonly removeLabel?: string;
  readonly disabled?: boolean;
}) {
  return (
    <span className="inline-flex h-6 max-w-full items-center gap-1 rounded bg-accent-soft px-2 text-xs text-accent">
      <span className="min-w-0 truncate">{children}</span>
      {onRemove && (
        <IconButton
          aria-label={removeLabel ?? "移除标签"}
          disabled={disabled}
          className="size-4 shrink-0 border-0 p-0 text-current"
          onClick={(event) => {
            event.stopPropagation();
            onRemove();
          }}
        >
          <X size={10} />
        </IconButton>
      )}
    </span>
  );
}
