import { useStore } from "zustand";
import { studio } from "../../../features/workflow";
import { Button, Toast } from "../../ui";

const dismiss = () => studio.message(null);

/** 工作流提示适配：保存冲突保留操作入口，不进入通用 UI 组件。 */
export function StudioToast() {
  const message = useStore(studio.store, (state) => state.message);
  const type = useStore(studio.store, (state) => state.messageType);
  const conflict = useStore(studio.store, (state) =>
    state.active ? state.tabs[state.active]?.status === "conflict" : false,
  );
  if (!message) return null;
  return (
    <Toast
      message={message}
      type={conflict ? "warning" : type}
      duration={conflict || type === "error" ? 0 : 4000}
      onClose={dismiss}
      actions={
        conflict ? (
          <>
            <Button onClick={() => void studio.safely(() => studio.saveCopy())}>
              保留为副本
            </Button>
            <Button onClick={() => void studio.safely(() => studio.reload())}>
              重新载入磁盘版本
            </Button>
          </>
        ) : undefined
      }
    />
  );
}
