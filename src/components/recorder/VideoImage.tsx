import type { RawInput } from "../../features/recorder/model";
import type { VideoFrame } from "../../features/recorder/video";
import { imageClickPoint } from "../../features/recorder/video-target";

/** 标记与原图共享等比例视口，保留原始PNG像素不变。 */
export function VideoImage({
  frame,
  point,
}: {
  readonly frame: VideoFrame;
  readonly point: RawInput["point"] | null;
}) {
  const local = imageClickPoint(point, frame.screen_origin, frame.image.size);
  return (
    <div className="relative h-full w-full">
      <img
        className="absolute inset-0 h-full w-full object-contain"
        src={frame.url}
        alt="按操作时间读取的录制画面"
      />
      {local && (
        <svg
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 h-full w-full"
          viewBox={`0 0 ${frame.image.size[0]} ${frame.image.size[1]}`}
        >
          <circle
            cx={local.x}
            cy={local.y}
            r="18"
            fill="none"
            stroke="white"
            strokeWidth="6"
          />
          <circle
            data-testid="recorded-click"
            cx={local.x}
            cy={local.y}
            r="18"
            fill="none"
            stroke="#e11d48"
            strokeWidth="3"
          />
          <path
            d={`M ${local.x - 25} ${local.y} h 12 M ${local.x + 13} ${local.y} h 12 M ${local.x} ${local.y - 25} v 12 M ${local.x} ${local.y + 13} v 12`}
            stroke="#e11d48"
            strokeWidth="3"
          />
        </svg>
      )}
    </div>
  );
}
