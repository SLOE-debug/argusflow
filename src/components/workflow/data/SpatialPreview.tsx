import { useId } from "react";
import type { Preview } from "../../../features/workflow";

function Plot({ preview: p }: { readonly preview: Preview }) {
  const clip = useId();
  const [x, y, w, h] = p.scope;
  const [ax, ay, aw, ah] = p.anchor;
  const cx = ax + aw / 2,
    cy = ay + ah / 2;
  const radius = Math.hypot(w, h) * 2;
  const point = (angle: number) =>
    [
      cx + radius * Math.cos((angle * Math.PI) / 180),
      cy - radius * Math.sin((angle * Math.PI) / 180),
    ].join(" ");
  const sector =
    p.angles &&
    `M ${cx} ${cy} L ${point(p.angles[0])} A ${radius} ${radius} 0 ${(p.angles[1] - p.angles[0] + 360) % 360 > 180 ? 1 : 0} 0 ${point(p.angles[1])} Z`;
  return (
    <div className="mt-3 space-y-2">
      <p className="text-xs text-muted">
        坐标空间：{p.space} · 蓝框为锚点，绿框为选中目标
      </p>
      <svg
        viewBox={`${x} ${y} ${w} ${h}`}
        role="img"
        aria-label="空间定位预览"
        className="max-h-80 w-full rounded border border-line bg-subtle"
      >
        <defs>
          <clipPath id={clip}>
            <rect x={x} y={y} width={w} height={h} />
          </clipPath>
        </defs>
        <g clipPath={`url(#${clip})`}>
          {sector && <path d={sector} fill="#60a5fa" opacity="0.15" />}
          <rect
            x={ax}
            y={ay}
            width={aw}
            height={ah}
            fill="none"
            stroke="#2563eb"
            strokeWidth="2"
            vectorEffect="non-scaling-stroke"
          />
          {p.candidates.map((c) => (
            <rect
              key={c.node}
              x={c.bounds[0]}
              y={c.bounds[1]}
              width={c.bounds[2]}
              height={c.bounds[3]}
              fill="none"
              stroke={c.selected ? "#16a34a" : "#94a3b8"}
              strokeWidth="2"
              vectorEffect="non-scaling-stroke"
            >
              <title>{`候选 ${c.node}：${c.reason ?? `距离 ${c.distance.toFixed(1)}，排序 ${c.rank ?? "未排序"}`}`}</title>
            </rect>
          ))}
        </g>
      </svg>
      <div className="max-h-48 overflow-auto">
        <table className="w-full text-left text-xs">
          <thead>
            <tr>
              <th>候选</th>
              <th>距离</th>
              <th>角度</th>
              <th>排序</th>
              <th>结果</th>
            </tr>
          </thead>
          <tbody>
            {p.candidates.map((c) => (
              <tr key={c.node}>
                <td>{c.node}</td>
                <td>{c.distance.toFixed(1)}</td>
                <td>{c.angle?.toFixed(1) ?? "中心重合"}</td>
                <td>{c.rank ?? "—"}</td>
                <td>{c.reason ?? (c.selected ? "选中" : "未选中")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function SpatialPreview({
  previews,
}: {
  readonly previews: readonly Preview[];
}) {
  return previews.map((preview, index) => (
    <Plot key={index} preview={preview} />
  ));
}
