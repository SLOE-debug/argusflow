import { useRef } from "react";
import { Button } from "./Button";

/** 通用文件选择控件，保持原生键盘和文件对话框行为。 */
export function FileInput({
  onFile,
  disabled,
  accept,
  label,
  buttonLabel,
}: {
  readonly onFile: (file: File) => void;
  readonly disabled?: boolean;
  readonly accept: string;
  readonly label: string;
  readonly buttonLabel: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <Button disabled={disabled} onClick={() => input.current?.click()}>
        {buttonLabel}
      </Button>
      <input
        ref={input}
        type="file"
        accept={accept}
        aria-label={label}
        className="hidden"
        onChange={(event) => {
          const file = event.currentTarget.files?.[0];
          if (file) onFile(file);
          event.currentTarget.value = "";
        }}
      />
    </>
  );
}
