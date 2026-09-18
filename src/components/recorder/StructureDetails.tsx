import { Collapse } from "../ui";
import type { RecordingRecord } from "../../features/recorder/model";
import type { ReactNode } from "react";
import {
  controlType,
  fieldLabel,
  fieldValue,
  structureGroups,
} from "./structurePresentation";

/** 常用信息直接展示，采集依据与原始属性收进详情。 */
export function StructureDetails({
  records,
  heading,
}: {
  readonly records: readonly RecordingRecord[];
  readonly heading?: ReactNode;
}) {
  const groups = structureGroups(records);
  const failures = records.flatMap(({ id, data }) =>
    "Attempt" in data &&
    data.Attempt.stage === "Uia" &&
    data.Attempt.outcome !== "Complete"
      ? [{ id, reason: data.Attempt.reason }]
      : [],
  );
  return (
    <aside
      aria-label="控件信息"
      className="w-80 shrink-0 overflow-auto border-l border-line bg-surface p-3 text-xs"
    >
      {heading}
      <h3 className="mb-2 font-semibold">操作对象</h3>
      {!groups.length && (
        <p className="text-muted">
          未读取到操作对象的信息，可以通过左侧画面查看。
        </p>
      )}
      {groups.length > 0 && (
        <p className="mb-3 text-muted">以下是录制时读取的控件信息。</p>
      )}
      {failures.map(({ id, reason }) => (
        <Collapse
          key={id}
          className="mb-2 text-warning"
          title={<>控件信息读取未完成，查看原因</>}
        >
          <p className="mt-2 break-words">{reason}</p>
        </Collapse>
      ))}
      {groups.map(({ structure, records: snapshots }) => (
        <section
          key={snapshots[0].id}
          className="mb-4 break-words rounded-md border border-line p-3"
        >
          <h4 className="font-semibold">
            {typeof structure.properties.name === "string" &&
            structure.properties.name
              ? structure.properties.name
              : "未命名控件"}{" "}
            · {controlType(structure)}
          </h4>
          {snapshots.length > 1 && (
            <p className="mt-1 text-muted">
              {snapshots.length} 次读取结果相同，已合并显示
            </p>
          )}
          {structure.stale ? (
            <p className="mt-2 text-warning">
              读取到的控件来自另一个进程，可能不是你当时操作的对象。请以左侧画面为准。
            </p>
          ) : (
            !structure.target_confirmed && (
              <p className="mt-2 text-muted">
                信息在操作后读取，可能与操作瞬间不同。
              </p>
            )
          )}
          {structure.truncated && (
            <p className="mt-2 text-warning">只读取了部分控件信息。</p>
          )}
          {structure.sensitive && (
            <p className="mt-2 text-muted">敏感内容已隐藏。</p>
          )}
          <dl className="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2">
            {["observed_value", "enabled", "focused"]
              .filter((name) => name in structure.properties)
              .map((name) => (
                <div key={name} className="contents">
                  <dt className="text-muted">{fieldLabel(name)}</dt>
                  <dd className="whitespace-pre-wrap break-all">
                    {structure.sensitive && name === "observed_value"
                      ? "已隐藏"
                      : fieldValue(structure.properties[name])}
                  </dd>
                </div>
              ))}
          </dl>
          <Collapse className="mt-3" title={<>技术详情</>}>
            <p className="my-2 text-muted">
              来源：
              {structure.source === "Uia"
                ? "Windows 辅助功能接口（UIA）"
                : "浏览器"}
            </p>
            <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2">
              {Object.entries({
                ...structure.identity,
                ...structure.properties,
              })
                .filter(
                  ([name]) =>
                    !["name", "observed_value", "enabled", "focused"].includes(
                      name,
                    ),
                )
                .map(([name, value]) => (
                  <div key={name} className="contents">
                    <dt className="text-muted">{fieldLabel(name)}</dt>
                    <dd className="break-all">{fieldValue(value)}</dd>
                  </div>
                ))}
              {structure.bounds && (
                <>
                  <dt className="text-muted">屏幕边界（左、上、右、下）</dt>
                  <dd>{structure.bounds.join(", ")}</dd>
                </>
              )}
            </dl>
            <p className="mt-3 text-muted">原始采集说明：{structure.scope}</p>
            <p className="mt-2 text-muted">
              记录编号：{snapshots.map((record) => record.id).join("、")}
            </p>
          </Collapse>
          {!!structure.ancestors.length && (
            <Collapse className="mt-3" title={<>所属窗口与容器</>}>
              {structure.ancestors.map((ancestor, index) => (
                <dl
                  key={index}
                  className="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-2 border-t border-line pt-2"
                >
                  {Object.entries(ancestor).map(([name, value]) => (
                    <div key={name} className="contents">
                      <dt className="text-muted">{fieldLabel(name)}</dt>
                      <dd className="break-all">{fieldValue(value)}</dd>
                    </div>
                  ))}
                </dl>
              ))}
            </Collapse>
          )}
        </section>
      ))}
    </aside>
  );
}
