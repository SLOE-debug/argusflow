import { Expand, X } from 'lucide-react';
import {
  useEffect,
  useId,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';

/** Inspector 结构化内容编辑器支持的两种布局。 */
export type InspectorEditorLayout = 'inline' | 'expanded';

export type InspectorEditorSectionProps = Readonly<{
  /** 编辑内容的业务标题。 */
  title: string;
  /** 标识内容语言或格式的短标签。 */
  badge: string;
  /** 标题栏中的领域操作。 */
  actions?: ReactNode;
  /** 编辑器下方的状态与辅助信息。 */
  footer?: ReactNode;
  /** 是否允许把同一编辑内容展开到宽抽屉。 */
  expandable?: boolean;
  /** 根据当前布局渲染同一份受控编辑内容。 */
  renderContent: (layout: InspectorEditorLayout) => ReactNode;
}>;

/** 为 Inspector 中的多行结构化内容提供标题、工具栏与展开抽屉。 */
export function InspectorEditorSection({
  title,
  badge,
  actions = null,
  footer = null,
  expandable = true,
  renderContent,
}: InspectorEditorSectionProps) {
  const [layout, setLayout] = useState<InspectorEditorLayout>('inline');
  const titleId = useId();

  useEffect(() => {
    if (!expandable) {
      setLayout('inline');
    }
  }, [expandable]);

  useEffect(() => {
    if (layout !== 'expanded') {
      return undefined;
    }
    /** Escape 在没有更高优先级编辑器浮层时关闭展开抽屉。 */
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) {
        setLayout('inline');
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [layout]);

  /** 两种布局共享的标题与领域操作，避免展开后出现第二套编辑状态。 */
  const header = (
    <EditorSectionHeader
      title={title}
      badge={badge}
      actions={actions}
      titleId={titleId}
      expanded={layout === 'expanded'}
      expandable={expandable}
      onExpand={() => setLayout('expanded')}
      onClose={() => setLayout('inline')}
    />
  );

  if (layout === 'expanded') {
    return createPortal(
      <div
        className="fixed inset-0 z-[80] flex justify-end bg-slate-950/25"
        role="presentation"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) {
            setLayout('inline');
          }
        }}
      >
        <section
          aria-labelledby={titleId}
          aria-modal="true"
          className="flex h-full w-[min(900px,calc(100vw-48px))] flex-col border-l border-slate-200 bg-white shadow-2xl"
          role="dialog"
        >
          {header}
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            <div className="mx-auto flex w-full max-w-[960px] flex-col gap-2.5">
              {renderContent(layout)}
              {footer}
            </div>
          </div>
        </section>
      </div>,
      document.body,
    );
  }

  return (
    <section className="flex flex-col gap-2 rounded-lg border border-slate-200 bg-slate-50/50 p-2.5 shadow-sm">
      {header}
      {renderContent(layout)}
      {footer}
    </section>
  );
}

type EditorSectionHeaderProps = Readonly<{
  title: string;
  badge: string;
  actions: ReactNode;
  titleId: string;
  expanded: boolean;
  expandable: boolean;
  onExpand: () => void;
  onClose: () => void;
}>;

/** 渲染内联区和展开抽屉共用的紧凑标题栏。 */
function EditorSectionHeader({
  title,
  badge,
  actions,
  titleId,
  expanded,
  expandable,
  onExpand,
  onClose,
}: EditorSectionHeaderProps) {
  return (
    <header className={expanded
      ? 'flex min-h-12 shrink-0 items-center gap-2 border-b border-slate-200 px-5 py-2'
      : 'flex min-h-7 flex-wrap items-center gap-2'}>
      <h3 id={titleId} className="text-[12px] font-semibold text-slate-800">
        {title}
      </h3>
      <span className="rounded bg-slate-200/70 px-1.5 py-0.5 font-mono text-[9px] font-medium text-slate-500">
        {badge}
      </span>
      <div className="ml-auto flex flex-wrap items-center justify-end gap-1">
        {actions}
        {expanded ? (
          <button
            type="button"
            aria-label={`关闭${title}展开编辑`}
            className="flex size-7 items-center justify-center rounded-md border border-slate-200 bg-white text-slate-500 hover:border-slate-300 hover:text-slate-800"
            onClick={onClose}
          >
            <X className="size-3.5" aria-hidden="true" />
          </button>
        ) : expandable ? (
          <button
            type="button"
            aria-label={`展开编辑${title}`}
            className="flex h-7 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600 hover:border-blue-300 hover:text-blue-700"
            onClick={onExpand}
          >
            <Expand className="size-3" aria-hidden="true" />
            展开编辑
          </button>
        ) : null}
      </div>
    </header>
  );
}
