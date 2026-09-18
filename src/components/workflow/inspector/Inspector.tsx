import { Collapse } from "../../ui";
import { ArrowUpRight, X } from "lucide-react";
import {
  nodeById,
  endpointKind,
  nodeKind,
  nodeTitle,
  setLayout,
  studio,
  type EditorTab,
} from "../../../features/workflow";
import { Button, Input, Textarea } from "../../ui";
import { NodeIcon } from "../presentation/NodeIcon";
import { NODE_CATALOG } from "../../../features/workflow";
import { nodeBadgeTone, nodeTone } from "../presentation/nodeTone";
import { TimeoutFields } from "./TimeoutFields";
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
  if (
    endpoint &&
    endpoint !== "start" &&
    tab.file.editor.nodes[tab.selected[0]]
  )
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
              : endpoint === "start"
                ? "节点属性"
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
        ) : endpoint === "start" ? (
          <div className="rounded-lg bg-subtle p-4">
            <p className="text-sm font-medium">开始节点无需配置</p>
            <p className="mt-2 text-xs leading-6 text-muted">
              请选择具有属性的节点，查看或修改配置。
            </p>
          </div>
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
              选择画布中的步骤查看设置。运行前填写的内容和需要保留的结果，可在「流程设置」中调整。
            </p>
            <Button onClick={() => studio.panel("data")}>
              流程设置
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
          <span
            className={
              "mt-2 inline-flex rounded px-2 py-0.5 text-[11px] font-medium " +
              nodeBadgeTone(nodeKind(node))
            }
            title={
              NODE_CATALOG.find((item) => item.id === nodeKind(node))
                ?.description
            }
          >
            {NODE_CATALOG.find((item) => item.id === nodeKind(node))?.title ??
              nodeKind(node)}
          </span>
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
          <TaskFields key={node.id} node={node} tab={tab} />
        ) : (
          <ControlFields key={node.id} node={node} tab={tab} />
        )}
        <OutputFields node={node} tab={tab} />
        <TimeoutFields node={node} tab={tab} />
        <Collapse
          className="group mt-4 border-t border-line pt-3"
          title="备注"
          extra="添加说明"
        >
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
        </Collapse>
      </fieldset>
    </aside>
  );
}
