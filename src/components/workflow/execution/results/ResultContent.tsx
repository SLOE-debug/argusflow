import {
  isSpatialPreview,
  parseSpatialPreview,
  type Value,
} from "../../../../features/workflow";
import { SpatialPreview } from "../../data";

/** 以正文、信息行和表格展示结果；协议类型不进入用户阅读区。 */
export function ResultContent({ value }: { readonly value: Value }) {
  if (isSpatialPreview(value)) {
    const previews = parseSpatialPreview(value);
    return previews ? (
      <SpatialPreview previews={previews} />
    ) : (
      <p role="alert" className="text-sm text-danger">
        无法显示位置示意，返回的内容不完整。
      </p>
    );
  }
  if (value.type === "optional")
    return value.value === null ? (
      <Empty />
    ) : (
      <ResultContent value={value.value} />
    );
  if (value.type === "bool")
    return (
      <span className="inline-flex rounded-md bg-accent-soft px-4 py-2 text-base font-medium text-accent">
        {value.value ? "是" : "否"}
      </span>
    );
  if (value.type === "record") {
    const fields = Object.entries(value.value);
    return fields.length ? (
      <dl className="divide-y divide-line">
        {fields.map(([name, item]) => (
          <div
            key={name}
            className="grid grid-cols-[minmax(80px,160px)_minmax(0,1fr)] gap-4 py-3 first:pt-0"
          >
            <dt className="break-words text-sm text-muted">{name}</dt>
            <dd className="min-w-0">
              <ResultContent value={item} />
            </dd>
          </div>
        ))}
      </dl>
    ) : (
      <Empty />
    );
  }
  if (value.type === "list") {
    if (!value.value.length) return <Empty />;
    /** 记录列表直接展示为表格；不要求用户逐项展开数据结构。 */
    const records = value.value.filter((item) => item.type === "record");
    if (records.length === value.value.length) {
      const columns = [
        ...new Set(records.flatMap((item) => Object.keys(item.value))),
      ];
      return (
        <div className="overflow-auto rounded-lg border border-line [scrollbar-width:thin]">
          <table className="w-full border-collapse text-left text-sm">
            <thead className="sticky top-0 bg-subtle text-muted">
              <tr>
                <th className="w-12 px-4 py-3 font-medium">序号</th>
                {columns.map((name) => (
                  <th key={name} className="px-4 py-3 font-medium">
                    {name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {records.map((item, index) => (
                <tr key={index} className="align-top even:bg-subtle/30">
                  <td className="px-4 py-3 tabular-nums text-muted">
                    {index + 1}
                  </td>
                  {columns.map((name) => (
                    <td key={name} className="min-w-28 max-w-96 px-4 py-3">
                      {item.value[name] ? (
                        <ResultContent value={item.value[name]} />
                      ) : (
                        "—"
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    }
    return (
      <ol className="divide-y divide-line">
        {value.value.map((item, index) => (
          <li key={index} className="flex gap-4 py-3 first:pt-0">
            <span className="w-6 shrink-0 text-sm tabular-nums text-muted">
              {index + 1}
            </span>
            <div className="min-w-0 flex-1">
              <ResultContent value={item} />
            </div>
          </li>
        ))}
      </ol>
    );
  }
  return (
    <p className="whitespace-pre-wrap break-words text-sm leading-6 [overflow-wrap:anywhere]">
      {String(value.value) || "空白内容"}
    </p>
  );
}

function Empty() {
  return <p className="text-sm text-muted">没有内容</p>;
}
