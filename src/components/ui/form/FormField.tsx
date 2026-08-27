import type { ReactNode } from 'react';

export type FormFieldProps = Readonly<{
  /** 面向用户展示的字段名称。 */
  label: string;
  /** 要关联的原生控件 ID。 */
  htmlFor: string;
  /** 控件下方的补充说明。 */
  description?: ReactNode;
  /** 控件下方的校验错误。 */
  error?: ReactNode;
  /** 是否在标签旁展示必填标记。 */
  required?: boolean;
  /** 表单控件或复合控件。 */
  children: ReactNode;
}>;

/** 提供统一标签、说明和错误布局的基础表单字段容器。 */
export function FormField({
  label,
  htmlFor,
  description,
  error,
  required = false,
  children,
}: FormFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label
        htmlFor={htmlFor}
        className="text-[12px] font-medium text-slate-700"
      >
        {label}
        {required ? (
          <span
            className="ml-1 text-rose-600"
            aria-hidden="true"
          >
            *
          </span>
        ) : null}
      </label>
      {children}
      {description ? (
        <p className="m-0 text-[11px] leading-4 text-slate-500">{description}</p>
      ) : null}
      {error ? (
        <p
          className="m-0 text-[11px] leading-4 text-rose-700"
          role="alert"
        >
          {error}
        </p>
      ) : null}
    </div>
  );
}
