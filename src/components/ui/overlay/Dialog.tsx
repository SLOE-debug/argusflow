import { X } from 'lucide-react';
import { forwardRef, useEffect, useId, useRef, type ReactNode } from 'react';

import { IconButton } from '../button/IconButton';

export type DialogProps = Readonly<{
  /** 当前是否打开。 */
  open: boolean;
  /** 打开状态变化回调。 */
  onOpenChange: (open: boolean) => void;
  /** 对话框标题。 */
  title: string;
  /** 标题下方的补充说明。 */
  description?: ReactNode;
  /** 对话框主体。 */
  children?: ReactNode;
  /** 对话框底部操作区。 */
  footer?: ReactNode;
  /** 关闭按钮的可访问名称。 */
  closeLabel?: string;
  /** 主体内容附加样式。 */
  className?: string;
}>;

/** 基于原生 dialog 的受控模态容器，集中处理 Escape、遮罩和焦点恢复。 */
export const Dialog = forwardRef<HTMLDialogElement, DialogProps>(function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  closeLabel = '关闭对话框',
  className = '',
}, forwardedRef) {
  const localRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const setDialogRef = (dialog: HTMLDialogElement | null) => {
    localRef.current = dialog;
    if (typeof forwardedRef === 'function') forwardedRef(dialog);
    else if (forwardedRef) forwardedRef.current = dialog;
  };

  useEffect(() => {
    const dialog = localRef.current;
    if (!dialog) return undefined;

    if (open) {
      previousFocusRef.current = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      if (!dialog.open) {
        dialog.showModal();
      }
      const timeoutId = window.setTimeout(() => {
        const initialFocus = dialog.querySelector<HTMLElement>('[data-dialog-initial-focus]')
          ?? dialog.querySelector<HTMLElement>('button, input, select, textarea, [tabindex]:not([tabindex="-1"])');
        initialFocus?.focus();
      }, 0);
      return () => window.clearTimeout(timeoutId);
    }

    if (dialog.open) {
      dialog.close();
    }
    previousFocusRef.current?.focus();
    previousFocusRef.current = null;
    return undefined;
  }, [open]);

  const close = () => onOpenChange(false);

  return (
    <dialog
      ref={setDialogRef}
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      className={`m-auto w-[min(100%-2rem,32rem)] rounded-lg border border-slate-200 bg-white p-0 text-slate-800 shadow-xl outline-none backdrop:bg-slate-900/30 ${className}`}
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
      onClose={close}
      onClick={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <header className="flex items-start gap-3 border-b border-slate-200 px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2
            id={titleId}
            className="m-0 text-[14px] font-semibold text-slate-900"
          >
            {title}
          </h2>
          {description ? (
            <div id={descriptionId} className="mt-1 text-[12px] leading-5 text-slate-500">
              {description}
            </div>
          ) : null}
        </div>
        <IconButton
          icon={X}
          label={closeLabel}
          size="compact"
          onClick={close}
        />
      </header>
      {children ? <div className="px-4 py-4">{children}</div> : null}
      {footer ? (
        <footer className="flex items-center justify-end gap-2 border-t border-slate-200 px-4 py-3">
          {footer}
        </footer>
      ) : null}
    </dialog>
  );
});
