import { useCallback, useRef, useState } from "react";
import {
  AlertCircle,
  ChevronDown,
  Circle,
  FolderOpen,
  Pause,
  Play,
  Square,
} from "lucide-react";
import { type useRecorder } from "../../features/recorder";
import { Button, Menu, type MenuItem } from "../ui";
import { recorderIndicator } from "./statusPresentation";

/** 标题栏仅展示录制动作和计时，详细进度通过录制面板查看。 */
export function RecorderMenu({
  recorder,
  onOpen,
}: {
  readonly recorder: ReturnType<typeof useRecorder>;
  readonly onOpen: () => void;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  /** 使用按钮的屏幕坐标定位公共菜单，避免标题栏裁切。 */
  const [anchor, setAnchor] = useState<{
    readonly x: number;
    readonly y: number;
    readonly initialFocus: "menu" | "first-item";
  } | null>(null);
  const phase = recorder.status?.phase;
  const active =
    phase === "Recording" || phase === "Paused" || phase === "Stopping";
  const indicator = recorderIndicator(
    recorder.status,
    recorder.error,
    recorder.busy,
  );
  const seconds = Math.floor((recorder.status?.elapsed_ms ?? 0) / 1000);
  const elapsed = `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`;
  const closeMenu = useCallback(() => {
    setAnchor(null);
    trigger.current?.focus();
  }, []);
  const items: readonly MenuItem[] = [
    ...(active
      ? []
      : [
          {
            type: "action" as const,
            label: "开始录制",
            icon: <Circle />,
            disabled: recorder.busy,
            action: () => void recorder.start(),
          },
        ]),
    ...(phase === "Recording" || phase === "Paused"
      ? [
          {
            type: "action" as const,
            label: phase === "Paused" ? "继续录制" : "暂停录制",
            icon: phase === "Paused" ? <Play /> : <Pause />,
            disabled: recorder.busy,
            action: () =>
              void recorder.transition(
                phase === "Paused" ? "Recording" : "Paused",
              ),
          },
        ]
      : []),
    ...(active
      ? [
          {
            type: "action" as const,
            label: "停止并保存",
            icon: <Square />,
            disabled: recorder.busy || phase === "Stopping",
            action: () => void recorder.transition("Stopped"),
          },
        ]
      : []),
    { type: "action", label: "查看录制", icon: <FolderOpen />, action: onOpen },
  ];
  return (
    <div className="flex shrink-0 items-center gap-1 text-xs text-ink">
      <Button
        ref={trigger}
        variant="ghost"
        aria-label="录制菜单"
        title={indicator.detail}
        aria-haspopup="menu"
        aria-expanded={anchor !== null}
        className={
          "h-6 gap-1.5 px-2 py-0 text-xs font-medium leading-4 aria-expanded:bg-hover " +
          (indicator.tone === "error"
            ? "text-danger"
            : indicator.tone === "warning"
              ? "text-warning"
              : "text-ink")
        }
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          if (anchor) closeMenu();
          else {
            const bounds = trigger.current?.getBoundingClientRect();
            if (bounds)
              setAnchor({
                x: bounds.left,
                y: bounds.bottom + 5,
                // 原生键盘或辅助技术触发的 click 没有鼠标点击次数。
                initialFocus: event.detail === 0 ? "first-item" : "menu",
              });
          }
        }}
      >
        {indicator.tone !== "normal" ? (
          <AlertCircle size={13} aria-hidden="true" className="shrink-0" />
        ) : (
          active && (
            <Circle
              size={8}
              aria-hidden="true"
              className={
                phase === "Recording"
                  ? "fill-danger text-danger"
                  : "fill-muted text-muted"
              }
            />
          )
        )}
        <span className="block leading-4">{indicator.label}</span>
        {active && (
          <span className="block font-mono leading-4 tabular-nums text-muted">
            {elapsed}
          </span>
        )}
        <ChevronDown
          size={14}
          aria-hidden="true"
          className="block shrink-0 text-muted"
        />
      </Button>
      {anchor && (
        <Menu {...anchor} label="录制" items={items} onClose={closeMenu} />
      )}
    </div>
  );
}
