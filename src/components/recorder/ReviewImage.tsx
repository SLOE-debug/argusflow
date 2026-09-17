import { useState } from "react";
import {
  clickPoint,
  useFrameImage,
  type ReviewFrame,
} from "../../features/recorder";
import { Button } from "../ui";
/** 完整事件画面只叠加点击点；不绘制推测的变化或控件范围。 */
export function ReviewImage({
  directory,
  frame,
}: {
  readonly directory: string;
  readonly frame: ReviewFrame;
}) {
  const [actualSize, setActualSize] = useState(false);
  const attachment = frame.visual.image;
  const image = useFrameImage(directory, attachment);
  const point = clickPoint(frame);
  const label =
    frame.visual.relation === "Response"
      ? "事件响应画面"
      : frame.event
        ? "事件关联画面"
        : "补充采样帧";
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-9 items-center gap-3 border-b border-line px-4 py-1 text-xs">
        <span className="mr-auto">{label}</span>
        {point && (
          <span className="text-muted">
            点击 ({Math.round(point.x)}, {Math.round(point.y)})
          </span>
        )}
        <Button
          variant="ghost"
          aria-pressed={actualSize}
          disabled={!image.url}
          onClick={() => setActualSize((v) => !v)}
        >
          {actualSize ? "适应窗口" : "原始大小"}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto bg-subtle p-3">
        {image.url && attachment ? (
          <svg
            role="img"
            aria-label={label}
            viewBox={`0 0 ${attachment.size[0]} ${attachment.size[1]}`}
            width={actualSize ? attachment.size[0] : "100%"}
            height={actualSize ? attachment.size[1] : "100%"}
            className="block"
          >
            <image
              href={image.url}
              width={attachment.size[0]}
              height={attachment.size[1]}
            />
            {point && (
              <g>
                <title>{`点击位置 x=${point.x}, y=${point.y}；标记不代表控件边界`}</title>
                <rect
                  x={Math.max(0, point.x - 8)}
                  y={Math.max(0, point.y - 8)}
                  width={
                    Math.min(attachment.size[0], point.x + 8) -
                    Math.max(0, point.x - 8)
                  }
                  height={
                    Math.min(attachment.size[1], point.y + 8) -
                    Math.max(0, point.y - 8)
                  }
                  fill="none"
                  stroke="#ef4444"
                  strokeWidth={2}
                  vectorEffect="non-scaling-stroke"
                />
                <circle cx={point.x} cy={point.y} r={2} fill="#ef4444" />
              </g>
            )}
          </svg>
        ) : (
          <div
            className="flex h-full items-center justify-center p-6 text-sm text-muted"
            role={image.error ? "alert" : "status"}
          >
            {image.error ||
              (frame.visual.decision === "SensitiveOmitted"
                ? "此帧涉及密码字段，未保存图片"
                : attachment
                  ? "正在读取画面…"
                  : "此帧没有图片附件")}
          </div>
        )}
      </div>
      <div className="flex h-7 shrink-0 items-center gap-2 border-t border-line px-4 text-[11px] text-muted">
        <span>
          {frame.visual.relation === "Response"
            ? "事件后短暂观察；不代表应用已完成处理"
            : frame.event
              ? "事件时刻最近可用的缓存帧"
              : "独立时间采样，不代表操作已完成"}
        </span>
        <span className="ml-auto">
          采样 {(frame.visual.acquired_ns / 1e9).toFixed(3)}s ·{" "}
          {attachment?.size.join(" × ")}
        </span>
      </div>
    </div>
  );
}
