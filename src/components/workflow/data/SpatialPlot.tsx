import { useId } from "react";
import type { Preview } from "../../../features/workflow";

/** 按实际对象范围取景，四周留白用于编号；完整范围由用户显式切换。 */
export function spatialViewport(
  preview: Preview,
): readonly [number, number, number, number] {
  const bounds = [
    preview.anchor,
    ...preview.candidates.map((item) => item.bounds),
  ];
  const left = Math.min(...bounds.map(([x]) => x));
  const top = Math.min(...bounds.map(([, y]) => y));
  const right = Math.max(...bounds.map(([x, , width]) => x + width));
  const bottom = Math.max(...bounds.map(([, y, , height]) => y + height));
  const padding = Math.max(12, Math.max(right - left, bottom - top) * 0.18);
  return [
    left - padding,
    top - padding,
    right - left + padding * 2,
    bottom - top + padding * 2,
  ];
}

/** 仅绘制位置示意；不将缺少原图的诊断数据伪装成窗口截图。 */
export function SpatialPlot({
  preview: p,
  full,
}: {
  readonly preview: Preview;
  readonly full: boolean;
}) {
  const clip = useId();
  const [x, y, w, h] = full ? p.scope : spatialViewport(p);
  const [ax, ay, aw, ah] = p.anchor;
  const cx = ax + aw / 2;
  const cy = ay + ah / 2;
  const radius = Math.hypot(w, h) * 2;
  const labelSize = Math.max(w / 36, h / 18);
  const point = (angle: number) =>
    [
      cx + radius * Math.cos((angle * Math.PI) / 180),
      cy - radius * Math.sin((angle * Math.PI) / 180),
    ].join(" ");
  const sector =
    p.angles &&
    `M ${cx} ${cy} L ${point(p.angles[0])} A ${radius} ${radius} 0 ${(p.angles[1] - p.angles[0] + 360) % 360 > 180 ? 1 : 0} 0 ${point(p.angles[1])} Z`;
  return (
    <svg
      viewBox={`${x} ${y} ${w} ${h}`}
      role="img"
      aria-label="空间定位预览"
      className="h-64 w-full"
    >
      <defs>
        <clipPath id={clip}>
          <rect x={x} y={y} width={w} height={h} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${clip})`}>
        {sector && <path d={sector} fill="#60a5fa" opacity="0.1" />}
        <rect
          x={ax}
          y={ay}
          width={aw}
          height={ah}
          rx={Math.min(aw, ah) * 0.1}
          fill="#2563eb"
          fillOpacity="0.08"
          stroke="#2563eb"
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
        >
          <title>参照位置</title>
        </rect>
        {p.candidates.map((item, index) => (
          <g key={item.node}>
            <rect
              x={item.bounds[0]}
              y={item.bounds[1]}
              width={item.bounds[2]}
              height={item.bounds[3]}
              fill={item.selected ? "#059669" : "#94a3b8"}
              fillOpacity="0.12"
              stroke={item.selected ? "#059669" : "#94a3b8"}
              strokeWidth={item.selected ? 2.5 : 1.5}
              vectorEffect="non-scaling-stroke"
            >
              <title>{`对象 ${index + 1}：${item.selected ? "已选中" : "未选中"}`}</title>
            </rect>
            <text
              x={item.bounds[0] - labelSize * 0.5}
              y={item.bounds[1] + item.bounds[3] / 2}
              fontSize={labelSize}
              textAnchor="end"
              dominantBaseline="middle"
              fill={item.selected ? "#059669" : "#64748b"}
              fontWeight="600"
            >
              {index + 1}
            </text>
          </g>
        ))}
      </g>
    </svg>
  );
}
