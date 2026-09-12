import {
  pointsBounds,
  SnapIndex,
  type FlowPoint,
  type FlowRect,
  type ViewportTransform,
} from "../../../flow";
import {
  buildCanvasScene,
  scopeElements,
  setLayout,
  studio,
  type CanvasScene,
  type WorkflowFile,
} from "../../../features/workflow";
import type { NodeDragPreview } from "./scene";

/** 节点拖动独立维护原始指针和同层对齐目标，预览不写入工作台。 */
export class NodeDragGesture {
  readonly kind = "nodes";
  private readonly bounds: FlowRect;
  private readonly snapping: SnapIndex;
  /** 最近一次原始客户端坐标，用于 Alt 切换时立即更新。 */
  private latestPoint: FlowPoint;
  private moved = false;

  get point(): FlowPoint {
    return this.latestPoint;
  }

  constructor(
    private readonly scene: CanvasScene,
    readonly file: WorkflowFile,
    readonly scope: string,
    readonly ids: readonly string[],
    private readonly start: FlowPoint,
    private readonly view: ViewportTransform,
  ) {
    this.latestPoint = start;
    const selected = new Set(ids);
    const elements = scopeElements(scene, scope);
    const moving = elements.filter((rect) => selected.has(rect.id));
    /** 多选以整体包围盒吸附，保持节点之间的相对位置。 */
    this.bounds = pointsBounds(
      moving.flatMap((rect) => [
        { x: rect.x, y: rect.y },
        { x: rect.x + rect.width, y: rect.y + rect.height },
      ]),
    );
    this.snapping = new SnapIndex(
      elements.filter((rect) => !selected.has(rect.id)),
    );
  }

  /** 每次从起点计算，吸附修正永远不会反馈给下一次鼠标位移。 */
  update(point: FlowPoint, disabled: boolean): NodeDragPreview {
    this.latestPoint = point;
    const delta = {
      x: (point.x - this.start.x) / this.view.zoom,
      y: (point.y - this.start.y) / this.view.zoom,
    };
    this.moved ||= Boolean(delta.x || delta.y);
    const result = disabled
      ? { delta, guides: [] }
      : this.snapping.snap(this.bounds, delta, this.view.zoom);
    return { scope: this.scope, ids: this.ids, ...result };
  }

  /** 松手按最终指针重新计算落点，仅提交一次编辑与撤销记录。 */
  commit(point: FlowPoint, disabled: boolean): void {
    const tab = studio.active;
    if (!this.moved || !tab || tab.file !== this.file || studio.readonly)
      return;
    const { delta } = this.update(point, disabled);
    if (!delta.x && !delta.y) return;
    const before = this.scene.scopes[this.scope].transform;
    studio.edit((file) =>
      this.ids.reduce((next, id) => {
        const origin = this.file.editor.nodes[id];
        return setLayout(next, id, {
          x: origin.x + delta.x,
          y: origin.y + delta.y,
        });
      }, file),
    );
    const after = buildCanvasScene(studio.active!.file).scopes[this.scope]
      .transform;
    /** 子图边界扩张可能重排祖先布局，补偿相机保持拖动落点稳定。 */
    studio.view({
      ...tab.viewport,
      x: tab.viewport.x + (before.x - after.x) * tab.viewport.zoom,
      y: tab.viewport.y + (before.y - after.y) * tab.viewport.zoom,
    });
  }
}
