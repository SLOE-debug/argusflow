import {
  useId,
  useLayoutEffect,
  useRef,
  type KeyboardEventHandler,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Button } from "./Button";
/** 原生 dialog 管理焦点约束和 Escape。 */
export function Dialog({
  title,
  children,
  onClose,
  wide = false,
  fullscreen = false,
  initialFocus,
  onKeyDown,
  headerActions,
}: {
  readonly title: string;
  readonly children: ReactNode;
  readonly onClose: () => void;
  readonly wide?: boolean;
  /** 工作区对话框使用可伸缩内容区，保留原生模态焦点约束。 */
  readonly fullscreen?: boolean;
  readonly initialFocus?: RefObject<HTMLElement | null>;
  readonly onKeyDown?: KeyboardEventHandler<HTMLDialogElement>;
  readonly headerActions?: ReactNode;
}) {
  const titleId = useId();
  const ref = useRef<HTMLDialogElement>(null);
  useLayoutEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    // 未指定输入目标时聚焦容器，不让 showModal 默认选中关闭等首个按钮。
    (initialFocus?.current ?? dialog)?.focus({ preventScroll: true });
    return () => {
      dialog?.close();
    };
  }, [initialFocus]);
  return createPortal(
    <dialog
      ref={ref}
      tabIndex={-1}
      aria-labelledby={titleId}
      onKeyDown={onKeyDown}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className={
        "fixed m-auto rounded-xl border border-line bg-surface p-0 text-ink shadow-2xl backdrop:bg-black/45 backdrop:backdrop-blur-sm " +
        (fullscreen
          ? "h-[calc(100vh-5rem)] w-[calc(100vw-2rem)] max-w-none max-h-none open:flex open:flex-col"
          : "max-h-[85vh] " + (wide ? "w-[720px]" : "w-[440px]"))
      }
    >
      <header
        role="presentation"
        className={
          "flex shrink-0 items-center gap-3 border-b border-line " +
          (fullscreen ? "px-4 py-1.5" : "px-5 py-3")
        }
      >
        <h2 id={titleId} className="shrink-0 text-sm font-semibold">
          {title}
        </h2>
        <div className="flex min-w-0 flex-1 items-center justify-end gap-2">
          {headerActions}
        </div>
        <Button
          variant="ghost"
          aria-label="关闭"
          onClick={onClose}
          className="size-7 shrink-0 p-0"
        >
          <X size={15} />
        </Button>
      </header>
      <div
        className={
          fullscreen
            ? "flex min-h-0 flex-1 flex-col overflow-hidden"
            : "max-h-[70vh] overflow-auto p-5"
        }
      >
        {children}
      </div>
    </dialog>,
    document.body,
  );
}
