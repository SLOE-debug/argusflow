import {
  BellRing,
  ChevronDown,
  ChevronUp,
  type LucideIcon,
} from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';

import type {
  ExecutionEvent,
  ValidationReport,
} from '../../../../features/workflow';
import type { WorkflowCanvasNode } from '../../../../features/workflow';
import { IconButton } from '../../../ui';
import { ExecutionLog } from '../../execution/ExecutionLog';
import { RunHistoryPanel } from '../../execution/RunHistoryPanel';
import type {
  StructuredEditorTarget,
  WorkspaceEditorMode,
} from './structuredEditorTarget';
import { WorkflowTaskTable } from '../../overview/WorkflowTaskTable';
import { WorkspaceDockResizeHandle } from './WorkspaceDockResizeHandle';
import { WorkspaceEditorHeader } from './WorkspaceEditorHeader';

type WorkspaceDockPanelProps = Readonly<{
  /** Dock 内容区是否展开。 */
  open: boolean;
  /** 当前结构化文档目标。 */
  editorTarget: StructuredEditorTarget | null;
  /** 当前结构化编辑器布局。 */
  editorMode: WorkspaceEditorMode;
  /** 当前 Dock 高度。 */
  dockHeight: number;
  /** 当前 Workspace 允许的 Dock 最小高度。 */
  minDockHeight: number;
  /** 当前 Workspace 允许的 Dock 最大高度。 */
  maxDockHeight: number;
  /** 双击 splitter 恢复的高度。 */
  defaultDockHeight: number;
  /** 已解析并受控的结构化编辑器内容。 */
  structuredEditor: ReactNode;
  /** 当前执行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 当前工作流节点，用于标题与执行日志。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 最近一次校验结果。 */
  report: ValidationReport | null;
  /** 展开或折叠整个 Dock。 */
  onOpenChange: (open: boolean) => void;
  /** 调整 Dock 高度。 */
  onDockHeightChange: (height: number) => void;
  /** 最大化或还原结构化编辑器。 */
  onEditorModeChange: (mode: WorkspaceEditorMode) => void;
  /** 关闭当前结构化文档。 */
  onCloseEditor: () => void;
}>;

type UtilityTab = 'tasks' | 'runs' | 'logs' | 'alerts';
type DockTab = 'structured_editor' | UtilityTab;

/** Utility Tabs 的稳定顺序与产品名称。 */
const UTILITY_TABS = [
  { id: 'tasks', label: '任务' },
  { id: 'runs', label: '运行记录' },
  { id: 'logs', label: '运行日志' },
  { id: 'alerts', label: '问题' },
] as const satisfies ReadonlyArray<Readonly<{ id: UtilityTab; label: string }>>;

/** 统一承载结构化编辑器、任务、运行记录、日志和告警的 Workspace Dock。 */
export function WorkspaceDockPanel({
  open,
  editorTarget,
  editorMode,
  dockHeight,
  minDockHeight,
  maxDockHeight,
  defaultDockHeight,
  structuredEditor,
  events,
  nodes,
  report,
  onOpenChange,
  onDockHeightChange,
  onEditorModeChange,
  onCloseEditor,
}: WorkspaceDockPanelProps) {
  const [activeTab, setActiveTab] = useState<DockTab>(() => (
    editorTarget ? 'structured_editor' : 'tasks'
  ));

  useEffect(() => {
    setActiveTab((current) => editorTarget
      ? 'structured_editor'
      : current === 'structured_editor' ? 'tasks' : current);
  }, [editorTarget]);

  const ToggleIcon = open ? ChevronDown : ChevronUp;
  const utilityTabs = (
    <UtilityTabButtons
      activeTab={activeTab}
      onActivate={(tab) => {
        setActiveTab(tab);
        if (editorMode === 'maximized') {
          onEditorModeChange('docked');
        }
        onOpenChange(true);
      }}
    />
  );
  const trailingActions = (
    <IconButton
      icon={ToggleIcon}
      label={open ? '收起底部面板' : '展开底部面板'}
      aria-expanded={open}
      onClick={() => onOpenChange(!open)}
    />
  );
  const editorNode = editorTarget
    ? (nodes.find((node) => node.id === editorTarget.nodeId) ?? null)
    : null;

  return (
    <section className="relative z-[18] flex h-full min-h-0 flex-col bg-white">
      {open && editorMode === 'docked' ? (
        <WorkspaceDockResizeHandle
          height={dockHeight}
          minHeight={minDockHeight}
          maxHeight={maxDockHeight}
          defaultHeight={defaultDockHeight}
          onHeightChange={onDockHeightChange}
        />
      ) : null}
      {editorTarget ? (
        <WorkspaceEditorHeader
          languageLabel={resolveEditorLanguage(editorTarget, editorNode)}
          nodeLabel={editorNode?.data.label ?? '节点不存在'}
          nodeId={editorTarget.nodeId}
          active={activeTab === 'structured_editor'}
          mode={editorMode}
          utilityTabs={utilityTabs}
          onActivate={() => {
            setActiveTab('structured_editor');
            onOpenChange(true);
          }}
          onModeChange={(mode) => {
            onEditorModeChange(mode);
            onOpenChange(true);
          }}
          onClose={onCloseEditor}
          trailingActions={trailingActions}
        />
      ) : (
        <header className="flex h-[38px] shrink-0 items-center border-b border-slate-200 bg-white px-2">
          {utilityTabs}
          <div className="ml-auto">{trailingActions}</div>
        </header>
      )}
      {open
        ? resolveDockContent(activeTab, structuredEditor, events, nodes, report)
        : null}
    </section>
  );
}

type UtilityTabButtonsProps = Readonly<{
  /** 当前激活的 Dock 页签。 */
  activeTab: DockTab;
  /** 激活一个 Utility Tab。 */
  onActivate: (tab: UtilityTab) => void;
}>;

/** 渲染结构化文档之外的稳定工具页签。 */
function UtilityTabButtons({ activeTab, onActivate }: UtilityTabButtonsProps) {
  return UTILITY_TABS.map((tab) => (
    <button
      key={tab.id}
      type="button"
      className={
        'relative flex h-[38px] items-center px-3 text-[12px] ' +
        (activeTab === tab.id
          ? 'font-semibold text-blue-600 after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-blue-600'
          : 'text-slate-500 hover:text-slate-800')
      }
      onClick={() => onActivate(tab.id)}
    >
      {tab.label}
    </button>
  ));
}

/** 按页签分派真实内容或明确占位页。 */
function resolveDockContent(
  activeTab: DockTab,
  structuredEditor: ReactNode,
  events: ReadonlyArray<ExecutionEvent>,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  report: ValidationReport | null,
) {
  switch (activeTab) {
    case 'structured_editor':
      return <div className="min-h-0 flex-1">{structuredEditor}</div>;
    case 'tasks':
      return <WorkflowTaskTable />;
    case 'logs':
      return <ExecutionLog events={events} nodes={nodes} report={report} />;
    case 'runs':
      return <RunHistoryPanel liveEvents={events} />;
    case 'alerts':
      return report?.issues.length ? (
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2">
            {report.issues.map((issue, index) => (
              <p
                key={`${issue.code}-${issue.node_id}-${index}`}
                className="mb-1 text-[11px] leading-5 text-rose-700 last:mb-0"
              >
                {issue.node_id ? `[${issue.node_id}] ` : ''}{issue.message}
              </p>
            ))}
          </div>
        </div>
      ) : (
        <DockPlaceholder
          icon={BellRing}
          title="暂无问题"
          description="工作流检查问题和运行异常会显示在这里。"
        />
      );
  }
}

/** 解析结构化文档的稳定语言标签。 */
function resolveEditorLanguage(
  target: StructuredEditorTarget,
  node: WorkflowCanvasNode | null,
): string {
  if (target.type === 'aql') return 'AQL';
  if (target.type === 'expression') return '表达式';
  if (node?.data.kind !== 'command') return '脚本';
  switch (node.data.operation.runner) {
    case 'power_shell':
      return 'PowerShell';
    case 'cmd':
      return 'CMD';
    case 'direct':
      return '脚本';
  }
}

type DockPlaceholderProps = Readonly<{
  icon: LucideIcon;
  title: string;
  description: string;
}>;

/** Workspace Utility Tab 的统一占位面板。 */
function DockPlaceholder({ icon: Icon, title, description }: DockPlaceholderProps) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center p-4">
      <div className="rounded-lg border border-dashed border-slate-300 bg-slate-50 px-8 py-5 text-center">
        <Icon className="mx-auto size-5 text-slate-400" aria-hidden="true" />
        <h3 className="mt-2 text-[12px] font-semibold text-slate-700">{title}</h3>
        <p className="mt-1 text-[11px] text-slate-500">{description}</p>
      </div>
    </div>
  );
}
