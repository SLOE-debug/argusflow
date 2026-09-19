import { useMemo, useState } from "react";
import { RecorderHistoryPopover } from "./RecorderHistoryPopover";
import { Dialog, Button } from "../ui";
import { AiDialog } from "../ai/AiDialog";
import { timeline, type useRecorder } from "../../features/recorder";
import { PlaybackWorkspace } from "./playback/PlaybackWorkspace";
type Controller = ReturnType<typeof useRecorder>;
/** 录制历史与时间线回看工作区。 */
export function RecorderPanel({
  recorder,
  onClose,
}: {
  readonly recorder: Controller;
  readonly onClose: () => void;
}) {
  const actions = useMemo(() => timeline(recorder.records), [recorder.records]);
  const directory = recorder.opened?.directory ?? "";
  const current = actions[0];
  const [aiOpened, setAiOpened] = useState(false);
  return (
    <Dialog
      title="录制回看"
      onClose={onClose}
      fullscreen
      headerActions={
        <>
          {recorder.opened && (
            <span className="mr-auto truncate text-xs text-muted">
              {new Date(recorder.opened.session.created_ms).toLocaleString()}
            </span>
          )}
          <RecorderHistoryPopover recorder={recorder} />
          <Button disabled={!directory} onClick={() => setAiOpened(true)}>
            AI 整理
          </Button>
        </>
      }
    >
      {aiOpened && (
        <AiDialog
          key={directory}
          directory={directory}
          onClose={() => setAiOpened(false)}
          onImported={onClose}
        />
      )}
      {(recorder.error || recorder.status?.input_fault) && (
        <p role="alert" className="px-4 py-2 text-xs text-danger">
          {recorder.error || recorder.status?.input_fault}
        </p>
      )}
      <div className="flex min-h-0 flex-1">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          {current && recorder.opened ? (
            <PlaybackWorkspace
              key={directory}
              action={current}
              records={recorder.records}
              directory={directory}
              frequency={recorder.opened.session.qpc_frequency}
            />
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted">
              {recorder.opened
                ? recorder.end
                  ? "此录制没有可回看的操作"
                  : "正在等待读取操作记录"
                : "选择录制历史，开始回看"}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
}
