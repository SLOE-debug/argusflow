import Braces from 'lucide-react/dist/esm/icons/braces.mjs';
import { useState } from 'react';

import type {
  WorkflowCanvasNode,
  WorkflowNodeData,
} from '../../../features/workflow';
import { Button, Dialog } from '../../ui';
import { InspectorSection } from './InspectorControls';

type NodeDeveloperSectionProps = Readonly<{
  /** 当前节点稳定标识。 */
  nodeId: string;
  /** 当前节点业务数据。 */
  data: WorkflowNodeData;
  /** 当前节点画布位置。 */
  position: WorkflowCanvasNode['position'];
  /** 当前节点卡片尺寸。 */
  size: WorkflowCanvasNode['size'];
  /** 当前节点类型的用户语言名称。 */
  nodeTypeLabel: string;
}>;

/** 直接展示紧凑技术摘要；大段 JSON 转入独立对话框，避免拉长编辑主路径。 */
export function NodeDeveloperSection({
  nodeId,
  data,
  position,
  size,
  nodeTypeLabel,
}: NodeDeveloperSectionProps) {
  const [configurationOpen, setConfigurationOpen] = useState(false);
  const runStateLabel = RUN_STATE_LABELS[data.runState ?? 'idle'];
  const configurationLabel = data.invalid ? '配置需修改' : '配置正常';
  const geometryLabel = [
    `${Math.round(position.x)}, ${Math.round(position.y)}`,
    `${size.width} × ${size.height}`,
  ].join(' · ');

  return (
    <>
      <InspectorSection
        title="开发者信息"
        action={(
          <Button
            variant="ghost"
            size="compact"
            icon={Braces}
            className="text-[11px] text-blue-600"
            onClick={() => setConfigurationOpen(true)}
          >
            查看配置
          </Button>
        )}
      >
        <dl
          className={
            'grid grid-cols-[2.75rem_minmax(0,1fr)] items-center gap-x-2 gap-y-1.5 ' +
            'rounded-md bg-slate-50 px-2.5 py-2 text-[11px] leading-4'
          }
        >
          <dt className="text-slate-400">状态</dt>
          <dd className="flex min-w-0 items-center gap-1.5 text-slate-700">
            <output aria-label="运行状态">{runStateLabel}</output>
            <span className="text-slate-300" aria-hidden="true">·</span>
            <output
              aria-label="配置检查"
              className={data.invalid ? 'text-rose-600' : 'text-emerald-600'}
            >
              {configurationLabel}
            </output>
          </dd>
          <dt className="text-slate-400">编号</dt>
          <dd className="min-w-0 truncate font-mono text-slate-700">
            <output aria-label="内部编号" title={nodeId}>{nodeId}</output>
          </dd>
          <dt className="text-slate-400">类型</dt>
          <dd className="min-w-0 truncate text-slate-700">
            <output aria-label="节点类型" title={nodeTypeLabel}>{nodeTypeLabel}</output>
          </dd>
          <dt className="text-slate-400">画布</dt>
          <dd className="min-w-0 truncate font-mono text-slate-700">
            <output aria-label="画布位置与卡片大小" title={geometryLabel}>
              {geometryLabel}
            </output>
          </dd>
        </dl>
      </InspectorSection>
      <Dialog
        open={configurationOpen}
        title="原始配置"
        description={`${nodeId} · ${nodeTypeLabel}`}
        closeLabel="关闭原始配置"
        className="w-[min(100%-2rem,44rem)]"
        onOpenChange={setConfigurationOpen}
      >
        <pre
          aria-label="节点原始配置"
          className={
            'max-h-[70vh] overflow-auto rounded-md bg-slate-950 p-3 font-mono ' +
            'text-[11px] leading-[18px] text-slate-200'
          }
        >
          {JSON.stringify(data, null, 2)}
        </pre>
      </Dialog>
    </>
  );
}

/** 节点运行状态的稳定中文名称。 */
const RUN_STATE_LABELS: Readonly<Record<NonNullable<WorkflowNodeData['runState']>, string>> = {
  idle: '等待执行',
  pending: '等待运行',
  running: '正在运行',
  success: '执行成功',
  error: '执行失败',
  skipped: '未执行',
};
