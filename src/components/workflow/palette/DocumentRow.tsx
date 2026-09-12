import { MoreHorizontal, Workflow } from "lucide-react";
import { Button, IconButton } from "../../ui";

/** 每行保留可发现的操作入口，键盘与右键使用同一组文档命令。 */
export function DocumentRow({
  id,
  name,
  active,
  disabled,
  locked,
  onOpen,
  onMenu,
  onRename,
  onDelete,
}: {
  readonly id: string;
  readonly name: string;
  readonly active: boolean;
  readonly disabled: boolean;
  readonly locked: boolean;
  readonly onOpen: () => void;
  readonly onMenu: (x: number, y: number) => void;
  readonly onRename: () => void;
  readonly onDelete: () => void;
}) {
  return (
    <div
      className={
        "group flex items-center rounded-md " +
        (active ? "bg-accent-soft text-accent" : "text-ink hover:bg-hover")
      }
      onContextMenu={(event) => {
        event.preventDefault();
        if (!disabled) onMenu(event.clientX, event.clientY);
      }}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (!locked && (event.key === "F2" || event.key === "Delete")) {
          event.preventDefault();
          event.stopPropagation();
          if (event.key === "F2") onRename();
          else onDelete();
        }
      }}
    >
      <Button
        variant="ghost"
        className="h-8 min-w-0 flex-1 justify-start gap-2 px-2 text-xs text-inherit"
        title={name}
        disabled={disabled}
        onClick={onOpen}
      >
        <Workflow size={13} className="shrink-0" />
        <span className="truncate">{name}</span>
      </Button>
      <IconButton
        aria-label={"工作流操作：" + name}
        aria-haspopup="menu"
        disabled={disabled}
        data-document-actions={id}
        className="mr-1 size-6 text-muted hover:text-ink"
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          onMenu(rect.left, rect.bottom + 4);
        }}
      >
        <MoreHorizontal size={15} />
      </IconButton>
    </div>
  );
}
