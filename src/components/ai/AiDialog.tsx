import { useAiAnalysis, useAiSettings } from "../../features/ai";
import { studio } from "../../features/workflow";
import { Button, Dialog } from "../ui";
import { AiSettings } from "./AiSettings";
import { AiResult } from "./AiResult";
const stages = {
  analyzing: "正在分析证据",
  inspecting: "正在补读证据",
  validating: "正在校验工作流",
} as const;
/** 录制分析和设置的装配入口；分析完成不会自动操作桌面。 */
export function AiDialog({
  directory,
  onClose,
  onImported,
}: {
  readonly directory?: string;
  readonly onClose: () => void;
  readonly onImported?: () => void;
}) {
  const settings = useAiSettings();
  const analysis = useAiAnalysis(directory);
  const ready =
    !!directory && settings.view?.has_key && !settings.dirty && !settings.busy;
  return (
    <Dialog
      title={directory ? "AI 整理工作流" : "AI 配置"}
      onClose={onClose}
      wide
    >
      <div className="space-y-5">
        <AiSettings settings={settings} disabled={analysis.running} />
        {directory && (
          <section className="space-y-3 border-t border-line pt-4">
            <p className="text-xs text-muted">
              将此录制的操作、控件、DOM、选区和剪贴板证据发送给已配置模型。图片可在上方关闭。请先停止录制。
            </p>
            <div className="flex items-center gap-3">
              <Button
                disabled={!ready || analysis.running}
                onClick={() => void analysis.run()}
              >
                开始整理
              </Button>
              {analysis.running && (
                <Button variant="ghost" onClick={() => void analysis.cancel()}>
                  取消分析
                </Button>
              )}
              {analysis.running && (
                <span role="status" className="text-xs text-muted">
                  第 {analysis.progress?.round ?? 1} 轮 ·{" "}
                  {analysis.progress
                    ? stages[analysis.progress.stage]
                    : "准备证据"}
                </span>
              )}
            </div>
            {settings.dirty && (
              <p className="text-xs text-muted">请先保存配置，再开始整理。</p>
            )}
            {analysis.error && (
              <p role="alert" className="text-xs text-danger">
                {analysis.error}
              </p>
            )}
          </section>
        )}
        {analysis.result && (
          <AiResult
            result={analysis.result}
            onImport={() => {
              const file = analysis.result?.file;
              if (file)
                void studio.safely(async () => {
                  studio.create(file);
                  await studio.flushAll();
                  onImported?.();
                  onClose();
                });
            }}
          />
        )}
      </div>
    </Dialog>
  );
}
