import ArrowRight from 'lucide-react/dist/esm/icons/arrow-right.mjs';

import type {
  ControlPortId,
  WorkflowCanvasEdge,
  WorkflowNodeData,
} from '../../../features/workflow';
import { Select } from '../../ui';
import { WORKFLOW_BRANCH_PRESENTATIONS } from '../presentation/workflowBranchPresentation';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorDeleteButton,
  InspectorField,
  InspectorSection,
} from './InspectorControls';

type EdgeInspectorFieldsProps = Readonly<{
  /** 当前选中的连线。 */
  edge: WorkflowCanvasEdge;
  /** 源节点决定当前端口族，不依赖端口字符串猜测语义。 */
  sourceData: WorkflowNodeData | null;
  /** 修改分支节点的控制端口。 */
  onBranchChange: (branch: ControlPortId) => void;
  /** 删除当前连线。 */
  onDelete: () => void;
}>;

/** 内置分支节点的控制端口及其用户可读名称。 */
const EDGE_BRANCH_OPTIONS = {
  boolean: [
    { value: 'true', label: WORKFLOW_BRANCH_PRESENTATIONS.true.text },
    { value: 'false', label: WORKFLOW_BRANCH_PRESENTATIONS.false.text },
    { value: 'unknown', label: WORKFLOW_BRANCH_PRESENTATIONS.unknown.text },
  ],
  observation: [
    { value: 'known', label: WORKFLOW_BRANCH_PRESENTATIONS.known.text },
    { value: 'unknown', label: WORKFLOW_BRANCH_PRESENTATIONS.unknown.text },
  ],
  loop: [
    { value: 'completed', label: '正常完成' },
    { value: 'exhausted', label: WORKFLOW_BRANCH_PRESENTATIONS.exhausted.text },
  ],
} as const;

/** 编辑当前选中边的分支控制端口。 */
export function EdgeInspectorFields({
  edge,
  sourceData,
  onBranchChange,
  onDelete,
}: EdgeInspectorFieldsProps) {
  const branchOptions = resolveBranchOptions(sourceData);
  return (
    <>
      <InspectorSection title="连线信息">
        <div className="flex items-center gap-2 rounded-md bg-slate-50 p-3 text-[11px] text-slate-600">
          <span className="min-w-0 flex-1 truncate">{edge.source.nodeId}</span>
          <ArrowRight className="size-4 shrink-0 text-blue-600" aria-hidden="true" />
          <span className="min-w-0 flex-1 truncate text-right">{edge.target.nodeId}</span>
        </div>
        {edge.data.branch && branchOptions ? (
          <InspectorField label="控制分支">
            <Select<ControlPortId>
              aria-label="控制分支"
              value={edge.data.branch}
              options={branchOptions}
              containerClassName="border-slate-300 bg-white"
              onValueChange={onBranchChange}
            />
          </InspectorField>
        ) : null}
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          拖动连线两端，可以更换起点或终点。
        </p>
      </InspectorSection>
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除连线" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}

/** 根据当前端口族返回可互换的分支，禁止跨节点语义改写端口。 */
function resolveBranchOptions(
  sourceData: WorkflowNodeData | null,
): ReadonlyArray<{ value: ControlPortId; label: string }> | null {
  if (sourceData?.kind === 'condition') return EDGE_BRANCH_OPTIONS.boolean.slice(0, 2);
  if (sourceData?.kind === 'loop') return EDGE_BRANCH_OPTIONS.loop;
  if (sourceData?.kind === 'observe') {
    return sourceData.resultType === 'boolean'
      ? EDGE_BRANCH_OPTIONS.boolean
      : EDGE_BRANCH_OPTIONS.observation;
  }
  return null;
}
