import { useState } from "react";
import { Check } from "lucide-react";
import type { Preview } from "../../../features/workflow";
import { Button, Collapse } from "../../ui";
import { SpatialPlot } from "./SpatialPlot";

/** 定位结果先展示目标和选中状态，数值诊断按需展开。 */
function PreviewItem({ preview }: { readonly preview: Preview }) {
  const [full, setFull] = useState(false);
  const selected = preview.candidates.filter((item) => item.selected).length;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-muted">
          找到 {preview.candidates.length} 个对象 · 已选中 {selected} 个
        </p>
        <Button variant="ghost" onClick={() => setFull(!full)}>
          {full ? "放大目标区域" : "显示整个范围"}
        </Button>
      </div>
      <div className="grid gap-4 @min-[680px]:grid-cols-[minmax(0,3fr)_minmax(200px,2fr)]">
        <div className="min-w-0 rounded-lg border border-line bg-subtle/30 p-4">
          <div className="mb-3 flex flex-wrap gap-4 text-xs text-muted">
            <span className="flex items-center gap-2">
              <span className="size-2 rounded-full bg-blue-600" />
              参照位置
            </span>
            <span className="flex items-center gap-2">
              <span className="size-2 rounded-full bg-emerald-600" />
              已选中
            </span>
            <span className="ml-auto">位置示意，不含原图</span>
          </div>
          <SpatialPlot preview={preview} full={full} />
        </div>
        <div className="max-h-80 space-y-2 overflow-auto [scrollbar-width:thin]">
          {!preview.candidates.length && (
            <p className="p-4 text-sm text-muted">没有找到符合条件的对象。</p>
          )}
          {preview.candidates.map((item, index) => (
            <div
              key={item.node}
              className={
                "flex items-center gap-3 rounded-lg border px-4 py-3 " +
                (item.selected
                  ? "border-emerald-600/25 bg-emerald-600/5"
                  : "border-line")
              }
            >
              <span
                className={
                  "flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-medium " +
                  (item.selected
                    ? "bg-emerald-600 text-white"
                    : "bg-subtle text-muted")
                }
              >
                {index + 1}
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">对象 {index + 1}</p>
                <p className="mt-1 break-words text-xs text-muted">
                  {item.reason ??
                    (item.selected ? "作为本次定位结果" : "未选中")}
                </p>
              </div>
              {item.selected && (
                <Check
                  size={16}
                  aria-label="已选中"
                  className="shrink-0 text-emerald-600"
                />
              )}
            </div>
          ))}
        </div>
      </div>
      <Collapse title="查看定位详情" className="border-t border-line pt-2">
        <div className="mt-2 overflow-auto rounded-lg border border-line [scrollbar-width:thin]">
          <table className="w-full text-left text-xs">
            <thead className="bg-subtle text-muted">
              <tr>
                {["对象", "距离", "角度", "排序", "结果"].map((label) => (
                  <th key={label} className="px-4 py-2 font-medium">
                    {label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {preview.candidates.map((item, index) => (
                <tr key={item.node}>
                  <td className="px-4 py-3">对象 {index + 1}</td>
                  <td className="px-4 py-3 tabular-nums">
                    {item.distance.toFixed(1)}
                  </td>
                  <td className="px-4 py-3 tabular-nums">
                    {item.angle === null
                      ? "中心重合"
                      : item.angle.toFixed(1) + "°"}
                  </td>
                  <td className="px-4 py-3">{item.rank ?? "—"}</td>
                  <td className="px-4 py-3">
                    {item.reason ?? (item.selected ? "选中" : "未选中")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-2 text-xs text-muted">
          坐标范围：{preview.space}；距离以该范围的坐标单位表示。
        </p>
      </Collapse>
    </div>
  );
}

/** 多组定位各自保留预览范围，避免相互覆盖。 */
export function SpatialPreview({
  previews,
}: {
  readonly previews: readonly Preview[];
}) {
  return (
    <div className="@container space-y-6">
      {previews.length ? (
        previews.map((preview, index) => (
          <section key={index}>
            {previews.length > 1 && (
              <h5 className="mb-3 text-sm font-medium">
                第 {index + 1} 组定位
              </h5>
            )}
            <PreviewItem preview={preview} />
          </section>
        ))
      ) : (
        <p className="text-sm text-muted">没有位置示意可显示。</p>
      )}
    </div>
  );
}
