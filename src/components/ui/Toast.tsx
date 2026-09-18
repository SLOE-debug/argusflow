import { useEffect, useState, type ReactNode } from "react";
import {
  CheckCircle2,
  AlertTriangle,
  CircleAlert,
  Info,
  X,
} from "lucide-react";
import { IconButton } from "./IconButton";

/** 提示语义；由调用方明确指定，不根据文字猜测级别。 */
export type ToastType = "success" | "warning" | "error" | "info";
const variants = {
  success: { icon: CheckCircle2, color: "text-success" },
  warning: { icon: AlertTriangle, color: "text-warning" },
  error: { icon: CircleAlert, color: "text-danger" },
  info: { icon: Info, color: "text-accent" },
} as const;

/** 非阻塞浮动提示；悬停或键盘聚焦暂停关闭，duration=0 时保持显示。 */
export function Toast({
  message,
  type = "info",
  duration = 4000,
  onClose,
  actions,
}: {
  readonly message: string;
  readonly type?: ToastType;
  readonly duration?: number;
  readonly onClose: () => void;
  readonly actions?: ReactNode;
}) {
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  useEffect(() => {
    if (duration <= 0 || hovered || focused) return;
    const timer = window.setTimeout(onClose, duration);
    return () => window.clearTimeout(timer);
  }, [message, type, duration, hovered, focused, onClose]);
  const variant = variants[type];
  const Icon = variant.icon;
  return (
    <div className="pointer-events-none fixed inset-x-4 top-12 z-[1000] flex justify-center">
      <div
        role={type === "error" ? "alert" : "status"}
        aria-atomic="true"
        className="pointer-events-auto flex max-w-lg items-start gap-3 rounded-lg border border-line bg-surface px-4 py-3 text-sm text-ink shadow-lg"
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onFocusCapture={() => setFocused(true)}
        onBlurCapture={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget))
            setFocused(false);
        }}
      >
        <Icon size={18} className={`mt-0.5 shrink-0 ${variant.color}`} />
        <div className="min-w-0 flex-1">
          <p className="whitespace-pre-wrap break-words">{message}</p>
          {actions && (
            <div className="mt-2 flex flex-wrap gap-2">{actions}</div>
          )}
        </div>
        <IconButton
          aria-label="关闭提示"
          className="-mr-1 -mt-1"
          onClick={onClose}
        >
          <X size={14} />
        </IconButton>
      </div>
    </div>
  );
}
