import {
  nodeKind,
  nodeTitle,
  nodeSummary,
  scopeEndpoints,
  nodeColor,
  type CanvasScene,
  type WorkflowFile,
  type NodeConnection,
} from "../../../features/workflow";
import type { FlowPoint, FlowAnchor, SnapGuide } from "../../../flow";
import type { ThemeToken } from "../../../features/themes";

/** 菜单、搜索与粘贴保留操作落点，不读取后续指针状态。 */
export interface CanvasLocation {
  readonly scope: string;
  readonly point: FlowPoint;
  readonly connection?: NodeConnection | null;
}
/** 工作流展示信息与几何分离，绘制热路径不查询领域模型。 */
export interface NodePresentation {
  readonly title: string;
  readonly summary: string;
  readonly kind: string;
  readonly tone: ThemeToken;
  readonly terminal: boolean;
}
/** 只绘制临时位移，提交前不修改文件和历史。 */
export interface NodeDragPreview {
  readonly scope: string;
  readonly ids: readonly string[];
  readonly delta: FlowPoint;
  readonly guides: readonly SnapGuide[];
}
/** Hover 不进入文件或撤销历史。 */
export interface CanvasHover {
  readonly scope: string;
  readonly kind: "node" | "edge";
  readonly id: string;
}
/** 临时线保留局部锚点，实际路由在绘制帧内计算。 */
export interface WirePreview {
  readonly scope: string;
  readonly source: FlowAnchor;
  readonly target: FlowAnchor;
  readonly replacing: string | null;
  readonly candidate: string | null;
  readonly error: string | null;
}
/** 工作流节点到可缓存画布展示模型的适配。 */
export function presentNodes(
  file: WorkflowFile,
): ReadonlyMap<string, NodePresentation> {
  const result = new Map<string, NodePresentation>(
    file.definition.scopes.flatMap((scope) =>
      scope.nodes.map((node) => {
        const kind = nodeKind(node);
        return [
          node.id,
          {
            title: nodeTitle(file, node),
            summary: nodeSummary(node),
            kind,
            tone: nodeColor(kind),
            terminal: ["return", "fail", "break", "continue"].includes(
              node.action.kind,
            ),
          } satisfies NodePresentation,
        ] as const;
      }),
    ),
  );
  for (const scope of file.definition.scopes)
    for (const endpoint of scopeEndpoints(file, scope.id))
      result.set(endpoint.id, {
        title: endpoint.kind === "start" ? "开始" : "结束",
        summary: endpoint.kind === "start" ? "工作流入口" : "工作流出口",
        kind: endpoint.kind,
        tone: nodeColor(endpoint.kind),
        terminal: endpoint.kind === "end",
      });
  return result;
}
/** 读取合法编辑作用域；历史删除容器后回到根作用域。 */
export function editingScope(scene: CanvasScene, scope: string) {
  return scene.scopes[scope] ?? scene.scopes[scene.root];
}
