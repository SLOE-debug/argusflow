import { PanelLeft, PanelRight } from "lucide-react";
import { studio, type EditorTab } from "../../../features/workflow";
import { ThemePicker } from "../../shell/ThemePicker";
import { IconButton } from "../../ui";

/** 工作区状态及视图偏好；面板隐藏后仍保留恢复入口。 */
export function StatusBar({
  tab,
  left,
  right,
  onLeft,
  onRight,
}: {
  readonly tab?: EditorTab;
  readonly left: boolean;
  readonly right: boolean;
  readonly onLeft: () => void;
  readonly onRight: () => void;
}) {
  return (
    <footer className="flex h-8 shrink-0 items-center gap-3 border-t border-line bg-panel px-3 text-[10px] text-muted">
      <span className="mr-auto">
        {tab
          ? tab.file.definition.scopes.reduce(
              (total, scope) => total + scope.nodes.length,
              0,
            ) +
            " 个节点 · " +
            (studio.readonly ? "运行快照只读" : "自动保存")
          : "本地工作区"}
      </span>
      {tab && (
        <div className="flex items-center gap-4 max-[1200px]:hidden">
          <span>Tab 添加</span>
          <span>空格 + 拖动平移</span>
          <span>Ctrl+C / Ctrl+V 复用</span>
        </div>
      )}
      <div className="flex items-center gap-0.5 border-l border-line pl-2">
        <IconButton
          aria-label="切换左侧面板"
          aria-pressed={left}
          className="size-6 aria-pressed:text-accent"
          onClick={onLeft}
        >
          <PanelLeft size={13} />
        </IconButton>
        {tab && (
          <IconButton
            aria-label="切换属性面板"
            aria-pressed={right}
            className="size-6 aria-pressed:text-accent"
            onClick={onRight}
          >
            <PanelRight size={13} />
          </IconButton>
        )}
      </div>
      <div className="border-l border-line pl-3">
        <ThemePicker />
      </div>
    </footer>
  );
}
