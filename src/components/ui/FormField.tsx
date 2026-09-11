import { useId, type ReactNode } from "react";

/** 字段标签可通过 htmlFor 关联控件；复杂字段作为具名组呈现。 */
export function FormField({
  label,
  children,
  error,
  stacked = false,
  htmlFor,
}: {
  readonly label: string;
  readonly children: ReactNode;
  readonly error?: string;
  readonly stacked?: boolean;
  readonly htmlFor?: string;
}) {
  const id = useId();
  return (
    <div
      className="min-w-0"
      role="group"
      aria-labelledby={id}
      aria-describedby={error ? id + "-error" : undefined}
    >
      <div
        className={
          stacked ? "space-y-2" : "flex min-h-8 min-w-0 items-start gap-3"
        }
      >
        <label
          id={id}
          htmlFor={htmlFor}
          className={
            "shrink-0 pt-1.5 text-xs text-muted " + (stacked ? "block" : "w-16")
          }
        >
          {label}
        </label>
        <div className="min-w-0 flex-1">{children}</div>
      </div>
      {error && (
        <p
          id={id + "-error"}
          role="alert"
          className="mt-1 text-[11px] text-danger"
        >
          {error}
        </p>
      )}
    </div>
  );
}
