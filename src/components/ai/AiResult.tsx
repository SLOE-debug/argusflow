import type { AiResult as Result } from "../../features/ai";
import { Button } from "../ui";
/** 可读证据引用和生成节点，编译通过与回放成功分别呈现。 */
export function AiResult({
  result,
  onImport,
}: {
  readonly result: Result;
  readonly onImport: () => void;
}) {
  return (
    <section className="space-y-3 border-t border-line pt-4">
      <p className="text-sm">{result.analysis.summary}</p>
      <p className="text-xs text-muted">
        分析 {result.metrics.rounds} 轮 · 工具 {result.metrics.tool_calls} 次 ·
        累计图片 {result.metrics.image_pixels.toLocaleString()} 像素 · 尚未回放
      </p>
      {result.analysis.unresolved.map((item, index) => (
        <p key={index} className="text-xs text-danger">
          待确认（证据 {item.evidence_ids.join(", ")}）：{item.reason}
        </p>
      ))}
      {result.analysis.required_bindings.map((item) => (
        <p key={item.kind + item.name} className="text-xs text-muted">
          需要{item.kind === "input" ? "输入" : "资源"} {item.name}：
          {item.reason}
        </p>
      ))}
      {result.file && (
        <>
          <p className="text-xs text-muted">
            已通过结构、任务编译和证据覆盖校验。导入后可在画布查看、修改并补齐运行绑定。
          </p>
          <ol className="max-h-56 space-y-2 overflow-auto text-xs">
            {result.file.definition.scopes
              .flatMap((scope) => scope.nodes)
              .map((node) => {
                const evidence = result.analysis.node_evidence.find(
                  (item) => item.node_id === node.id,
                );
                return (
                  <li key={node.id} className="rounded border border-line p-2">
                    {node.id} ·{" "}
                    {node.action.kind === "task"
                      ? node.action.task.type_id
                      : node.action.kind}
                    <div className="mt-1 text-muted">
                      证据 {evidence?.evidence_ids.join(", ")} ·{" "}
                      {evidence?.outcome}
                      {evidence?.uncertainty && " · " + evidence.uncertainty}
                    </div>
                  </li>
                );
              })}
          </ol>
          <Button onClick={onImport}>导入工作流画布</Button>
        </>
      )}
    </section>
  );
}
