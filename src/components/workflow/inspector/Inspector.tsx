import { ArrowUpRight, ChevronRight, X } from "lucide-react";
import {
  nodeById,
  endpointKind,
  nodeKind,
  nodeTitle,
  setLayout,
  studio,
  type EditorTab,
} from "../../../features/workflow";
import { Button, Input, Textarea, FormField } from "../../ui";
import { NodeIcon } from "../presentation/NodeIcon";
import { NODE_CATALOG, updateNode } from "../../../features/workflow";
import { nodeTone } from "../presentation/nodeTone";
import { TaskFields } from "./TaskFields";
import { ControlFields } from "./ControlFields";
import { OutputFields } from "./OutputFields";
import { arrangeSelection } from "../../../features/workflow/studio/arrangement";
import { EndpointInspector } from "./EndpointInspector";
/** 主配置按任务语义排布，次要执行参数按需展开。 */
export function Inspector({
  tab,
  onClose,
}: {
  readonly tab: EditorTab;
  readonly onClose: () => void;
}) {
  const node =
    tab.selected.length === 1 ? nodeById(tab.file, tab.selected[0]) : undefined;
  const endpoint =
    tab.selected.length === 1 ? endpointKind(tab.scope, tab.selected[0]) : null;
  if (endpoint && tab.file.editor.nodes[tab.selected[0]])
    return <EndpointInspector kind={endpoint} tab={tab} onClose={onClose} />;
  const pending = node
    ? Object.entries(tab.file.editor.drafts).filter(([key]) =>
        key.startsWith(node.id + ":"),
      )
    : [];
  if (!node)
    return (
      <aside className="h-full overflow-auto bg-surface p-5">
        <div className="mb-6 flex items-center justify-between">
          <h2 className="text-sm font-semibold">
            {tab.selected.length > 1
              ? "选择了 " + tab.selected.length + " 个节点"
              : "工作流"}
          </h2>
          <Button variant="ghost" aria-label="关闭属性面板" onClick={onClose}>
            <X size={14} />
          </Button>
        </div>
        {tab.selected.length > 1 ? (
          <>
            <p className="text-xs leading-6 text-muted">
              拖动任一选中节点可一起移动。右键菜单提供对齐、分布、复制和删除。
            </p>
            <div className="mt-4 flex flex-wrap gap-2">
              <Button onClick={() => studio.duplicate()}>创建副本</Button>
              <Button onClick={() => arrangeSelection("left")}>左对齐</Button>
              <Button onClick={() => arrangeSelection("top")}>顶部对齐</Button>
              <Button
                disabled={tab.selected.length < 3}
                onClick={() => arrangeSelection("horizontal")}
              >
                水平分布
              </Button>
            </div>
          </>
        ) : (
          <>
            <Input
              aria-label="工作流名称"
              className="mb-3 w-full bg-transparent text-sm font-semibold"
              value={tab.file.definition.name}
              disabled={studio.readonly}
              onChange={(event) =>
                studio.edit((file) => ({
                  ...file,
                  definition: { ...file.definition, name: event.target.value },
                }))
              }
            />
            <p className="mb-5 text-[11px] leading-5 text-muted">
              选择画布节点查看配置。流程输入、输出和资源端口集中在底部管理。
            </p>
            <Button onClick={() => studio.panel("data")}>
              输入输出
              <ArrowUpRight size={13} />
            </Button>
          </>
        )}
      </aside>
    );
  return (
    <aside className="flex h-full min-h-0 flex-col bg-surface">
      <header className="flex items-start gap-2.5 px-5 pb-5 pt-5">
        <span className={"mt-1.5 " + nodeTone(nodeKind(node))}>
          <NodeIcon kind={nodeKind(node)} size={20} />
        </span>
        <div className="min-w-0 flex-1">
          <Input
            data-node-title
            aria-label="节点名称"
            className="-ml-2 w-full bg-transparent text-sm font-semibold"
            value={nodeTitle(tab.file, node)}
            disabled={studio.readonly}
            onChange={(event) => studio.rename(node.id, event.target.value)}
          />
          <p className="mt-0.5 text-[10px] text-muted">
            {
              NODE_CATALOG.find((item) => item.id === nodeKind(node))
                ?.description
            }
          </p>
        </div>
        <Button
          variant="ghost"
          aria-label="关闭属性面板"
          className="-mr-2 h-7 px-1.5"
          onClick={onClose}
        >
          <X size={14} />
        </Button>
      </header>
      <fieldset
        disabled={studio.readonly}
        className="min-h-0 flex-1 overflow-auto px-5 pb-5 disabled:opacity-80"
      >
        {pending.length > 0 && (
          <div className="mb-4 rounded-md bg-danger-soft px-3 py-2 text-[11px] text-danger">
            {pending.map(([key]) => (
              <p key={key}>待完成：{key.split(":").slice(1).join(":")}</p>
            ))}
          </div>
        )}
        {node.action.kind === "task" ? (
          <TaskFields node={node} tab={tab} />
        ) : (
          <ControlFields node={node} tab={tab} />
        )}
        <OutputFields node={node} tab={tab} />
        <details className="group mt-6 border-t border-line pt-3">
          <summary className="flex cursor-pointer list-none items-center gap-2 text-xs">
            <ChevronRight
              size={12}
              className="transition-transform group-open:rotate-90"
            />
            执行设置
            <span className="ml-auto text-[10px] text-muted">
              {node.timeout_ms
                ? "超时 " + Number(node.timeout_ms) / 1000 + " 秒"
                : "使用流程时限"}
            </span>
          </summary>
          <div className="mt-3">
            <FormField label="超时">
              <Input
                aria-label="节点超时毫秒"
                className="w-24"
                placeholder="默认"
                value={
                  tab.file.editor.drafts[node.id + ":timeout"] ??
                  node.timeout_ms ??
                  ""
                }
                onChange={(event) => {
                  const value = event.target.value;
                  if (
                    !value ||
                    (/^\d+$/.test(value) &&
                      BigInt(value) <= 18446744073709551615n)
                  )
                    studio.draft(node.id, "timeout", null, (file) =>
                      updateNode(file, node.id, (current) => ({
                        ...current,
                        timeout_ms: value || null,
                      })),
                    );
                  else studio.draft(node.id, "timeout", value);
                }}
              />
              <span className="ml-2 text-xs text-muted">ms</span>
            </FormField>
          </div>
        </details>
        <details className="group mt-4 border-t border-line pt-3">
          <summary className="flex cursor-pointer list-none items-center gap-2 text-xs">
            <ChevronRight
              size={12}
              className="transition-transform group-open:rotate-90"
            />
            备注<span className="ml-auto text-[10px] text-muted">添加说明</span>
          </summary>
          <Textarea
            aria-label="节点备注"
            className="mt-3 h-20 w-full"
            value={tab.file.editor.nodes[node.id]?.note ?? ""}
            onChange={(event) =>
              studio.edit((file) =>
                setLayout(file, node.id, { note: event.target.value }),
              )
            }
          />
        </details>
      </fieldset>
    </aside>
  );
}
