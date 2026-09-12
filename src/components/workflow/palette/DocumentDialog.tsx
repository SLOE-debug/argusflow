import { useRef, useState } from "react";
import { studio } from "../../../features/workflow";
import { Button, Dialog, FormField, Input } from "../../ui";

export interface DocumentAction {
  readonly kind: "rename" | "delete";
  readonly id: string;
  readonly name: string;
}

/** 文档修改的表单和错误留在当前对话框，失败时不丢失输入。 */
export function DocumentDialog({
  action,
  onClose,
}: {
  readonly action: DocumentAction;
  readonly onClose: () => void;
}) {
  const [name, setName] = useState(action.name);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const rename = action.kind === "rename";
  const input = useRef<HTMLInputElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const submitting = useRef(false);
  const close = () => {
    if (!submitting.current) onClose();
  };
  const submit = async () => {
    if (submitting.current) return;
    submitting.current = true;
    setPending(true);
    setError(null);
    try {
      if (rename) await studio.renameDocument(action.id, name);
      else await studio.deleteDocument(action.id);
      onClose();
    } catch (error) {
      setError(String(error));
    } finally {
      submitting.current = false;
      setPending(false);
    }
  };
  return (
    <Dialog
      title={rename ? "重命名工作流" : "删除工作流"}
      initialFocus={rename ? input : cancelButton}
      onClose={close}
      onKeyDown={(event) => {
        if (
          rename ||
          event.nativeEvent.isComposing ||
          event.repeat ||
          event.ctrlKey ||
          event.metaKey ||
          event.altKey
        )
          return;
        const key = event.key.toLowerCase();
        if (key !== "y" && key !== "n") return;
        event.preventDefault();
        event.stopPropagation();
        if (key === "y") void submit();
        else close();
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        {rename ? (
          <FormField label="工作流名称">
            <Input
              aria-label="工作流名称"
              className="w-full"
              value={name}
              disabled={pending}
              ref={input}
              onFocus={(event) => event.currentTarget.select()}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && event.nativeEvent.isComposing)
                  event.preventDefault();
              }}
            />
          </FormField>
        ) : (
          <p className="break-words text-sm leading-6">
            删除“{action.name}
            ”及其本地文件？此操作无法撤销，已打开的标签也会关闭。
          </p>
        )}
        {error && (
          <p role="alert" className="mt-3 text-xs text-danger">
            {error}
          </p>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <Button
            ref={cancelButton}
            disabled={pending}
            onClick={close}
            aria-keyshortcuts={rename ? undefined : "N"}
          >
            <span className="inline-flex h-4 items-center leading-none">
              取消
            </span>
            {!rename && (
              <kbd
                aria-hidden="true"
                className="inline-flex h-4 items-center font-sans text-xs font-medium leading-none opacity-70"
              >
                N
              </kbd>
            )}
          </Button>
          <Button
            type="submit"
            variant={rename ? "primary" : "danger"}
            disabled={pending || (rename && !name.trim())}
            aria-keyshortcuts={rename ? undefined : "Y"}
          >
            <span className="inline-flex h-4 items-center leading-none">
              {pending
                ? rename
                  ? "保存中…"
                  : "删除中…"
                : rename
                  ? "保存名称"
                  : "删除"}
            </span>
            {!rename && !pending && (
              <kbd
                aria-hidden="true"
                className="inline-flex h-4 items-center font-sans text-xs font-medium leading-none opacity-70"
              >
                Y
              </kbd>
            )}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
