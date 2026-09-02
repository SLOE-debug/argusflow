import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import Maximize2 from 'lucide-react/dist/esm/icons/maximize-2.mjs';
import Minimize2 from 'lucide-react/dist/esm/icons/minimize-2.mjs';
import X from 'lucide-react/dist/esm/icons/x.mjs';
import type { ReactNode } from 'react';

import { IconButton } from '../../../ui';
import type { WorkspaceEditorMode } from './structuredEditorTarget';

type WorkspaceEditorHeaderProps = Readonly<{
  /** 文档语言标签。 */
  languageLabel: string;
  /** 所属节点的用户名称。 */
  nodeLabel: string;
  /** 所属节点稳定 ID。 */
  nodeId: string;
  /** 当前是否正在展示文档页。 */
  active: boolean;
  /** 当前中央工作区布局。 */
  mode: WorkspaceEditorMode;
  /** 普通任务、日志等 Utility Tabs。 */
  utilityTabs: ReactNode;
  /** 激活当前文档。 */
  onActivate: () => void;
  /** 切换最大化或还原。 */
  onModeChange: (mode: WorkspaceEditorMode) => void;
  /** 关闭当前文档。 */
  onClose: () => void;
  /** Dock 折叠按钮等末尾动作。 */
  trailingActions: ReactNode;
}>;

/** 将结构化文档、Utility Tabs 和非模态布局命令编排到同一 Dock 标题栏。 */
export function WorkspaceEditorHeader({
  languageLabel,
  nodeLabel,
  nodeId,
  active,
  mode,
  utilityTabs,
  onActivate,
  onModeChange,
  onClose,
  trailingActions,
}: WorkspaceEditorHeaderProps) {
  const ModeIcon = mode === 'maximized' ? Minimize2 : Maximize2;
  const modeLabel = mode === 'maximized' ? '还原编辑器' : '最大化编辑器';

  return (
    <header className="flex h-[38px] shrink-0 items-stretch border-b border-slate-200 bg-white px-2">
      <button
        type="button"
        className={
          'relative flex h-full min-w-0 max-w-[340px] items-center gap-1.5 px-3 ' +
          'text-[11px] leading-none ' +
          (active
            ? 'font-semibold text-blue-600 after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-blue-600'
            : 'text-slate-600 hover:text-slate-900')
        }
        title={`${languageLabel} · ${nodeLabel}\n${nodeId}`}
        onClick={onActivate}
      >
        <span className="inline-flex h-full shrink-0 items-center font-mono text-[9px] leading-none">
          {languageLabel}
        </span>
        <ChevronRight
          aria-hidden="true"
          className="size-3 shrink-0 text-slate-300"
        />
        <span className="inline-flex h-full min-w-0 items-center truncate leading-none">
          {nodeLabel}
        </span>
        <span
          className={
            'hidden h-full min-w-0 items-center truncate font-mono text-[9px] ' +
            'font-normal leading-none text-slate-400 xl:inline-flex'
          }
        >
          {nodeId}
        </span>
      </button>
      {utilityTabs}
      <div className="ml-auto flex h-full items-center gap-0.5">
        {active ? (
          <IconButton
            icon={ModeIcon}
            label={modeLabel}
            size="compact"
            onClick={() => onModeChange(mode === 'maximized' ? 'docked' : 'maximized')}
          />
        ) : null}
        <IconButton
          icon={X}
          label="关闭编辑器"
          onClick={onClose}
          className="hover:bg-rose-50 hover:text-rose-600"
        />
        {trailingActions}
      </div>
    </header>
  );
}
