import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Button } from "./Button";
/** 原生 dialog 管理焦点约束和 Escape。 */
export function Dialog({
  title,
  children,
  onClose,
  wide = false,
}: {
  readonly title: string;
  readonly children: ReactNode;
  readonly onClose: () => void;
  readonly wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => {
      dialog?.close();
    };
  }, []);
  return createPortal(
    <dialog
      ref={ref}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className={
        "fixed m-auto max-h-[85vh] rounded-xl border border-line bg-surface p-0 text-ink shadow-2xl backdrop:bg-black/30 " +
        (wide ? "w-[720px]" : "w-[440px]")
      }
    >
      <header className="flex items-center justify-between border-b border-line px-5 py-3">
        <h2 className="text-sm font-semibold">{title}</h2>
        <Button variant="ghost" aria-label="关闭" onClick={onClose}>
          <X size={15} />
        </Button>
      </header>
      <div className="max-h-[70vh] overflow-auto p-5">{children}</div>
    </dialog>,
    document.body,
  );
}
