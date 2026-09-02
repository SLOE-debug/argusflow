import type { ReactNode } from 'react';

import type {
  WorkflowCanvasNode,
  WorkflowNodeUpdater,
} from '../../../features/workflow';
import {
  InspectorDeleteButton,
  InspectorSection,
} from './InspectorControls';
import { NodeDeveloperSection } from './NodeDeveloperSection';
import { NodeInspectorHeader } from './NodeInspectorHeader';
import { NodeOutputSection } from './NodeOutputSection';

type GenericNodeInspectorProps = Readonly<{
  /** 当前节点完整画布快照。 */
  node: WorkflowCanvasNode;
  /** 当前节点类型的用户语言名称。 */
  nodeTypeLabel: string;
  /** 主设置分组标题。 */
  settingsTitle: string;
  /** 节点用途摘要。 */
  summary: string;
  /** 节点类型专属设置。 */
  children: ReactNode;
  /** 通过统一 Flow 事务写回节点。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  /** 删除当前节点。 */
  onDelete: () => void;
}>;

/** 非专用节点也使用统一意图头部与一级信息架构。 */
export function GenericNodeInspector({
  node,
  nodeTypeLabel,
  settingsTitle,
  summary,
  children,
  onUpdate,
  onDelete,
}: GenericNodeInspectorProps) {
  return (
    <>
      <NodeInspectorHeader
        label={node.data.label}
        summary={summary}
        runState={node.data.runState ?? 'idle'}
        invalid={node.data.invalid ?? false}
        onLabelChange={(label) => onUpdate((current) => ({ ...current, label }))}
      />
      <InspectorSection title={settingsTitle}>
        {children}
      </InspectorSection>
      <NodeOutputSection data={node.data} onUpdate={onUpdate} />
      <NodeDeveloperSection
        nodeId={node.id}
        data={node.data}
        position={node.position}
        size={node.size}
        nodeTypeLabel={nodeTypeLabel}
      />
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除节点" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}
