import type { WorkflowCanvasNode } from '../../../features/workflow';
import { getNodeValueOutputs } from '../../../features/workflow';

/** 展示节点自动发布到工作流值空间的只读输出目录。 */
export function WorkflowNodeOutputsTable({
  nodes,
}: Readonly<{ nodes: ReadonlyArray<WorkflowCanvasNode> }>) {
  const rows = nodes.flatMap((node) => getNodeValueOutputs(node.data).map((output) => ({ node, output })));
  return (
    <section className="flex min-h-0 flex-col gap-3">
      <div>
        <h3 className="text-[13px] font-semibold text-slate-800">节点输出</h3>
        <p className="mt-1 text-[11px] text-slate-500">节点成功执行后可供后续节点选择，输出名称保持稳定。</p>
      </div>
      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed border-slate-300 px-4 py-6 text-center text-[11px] text-slate-500">
          还没有可发布的节点输出。
        </div>
      ) : (
        <div className="overflow-x-auto rounded-md border border-slate-200">
          <table className="w-full min-w-[520px] text-left text-[11px]">
            <thead className="bg-slate-50 text-slate-500">
              <tr>
                <th className="px-3 py-2 font-medium">节点</th>
                <th className="px-3 py-2 font-medium">输出</th>
                <th className="px-3 py-2 font-medium">类型</th>
                <th className="px-3 py-2 font-medium">稳定引用</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {rows.map(({ node, output }) => (
                <tr key={`${node.id}:${output.name}`} className="text-slate-700">
                  <td className="px-3 py-2">
                    <span className="block font-medium">{node.data.label}</span>
                    <span className="font-mono text-[10px] text-slate-400">{node.id}</span>
                  </td>
                  <td className="px-3 py-2">{output.label}</td>
                  <td className="px-3 py-2 text-slate-500">{output.valueType === 'text' ? '文本' : 'JSON'}</td>
                  <td className="px-3 py-2 font-mono text-[10px] text-slate-500">{output.name}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
