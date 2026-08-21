import type { ValidationReport } from '../../features/workflow/contracts';

/** 工作流在桌面外壳中使用的紧凑状态展示。 */
export type WorkflowStatusPresentation = Readonly<{
  /** 面向用户的状态文字。 */
  label: string;
  /** 状态点使用的 Tailwind 色彩类。 */
  tone: string;
}>;

/** 将运行、校验和命令错误归一为桌面外壳状态。 */
export function resolveWorkflowStatus(
  running: boolean,
  report: ValidationReport | null,
  errorMessage: string | null,
): WorkflowStatusPresentation {
  if (running) return { label: '运行中', tone: 'bg-blue-500' };
  if (errorMessage) return { label: '发生错误', tone: 'bg-rose-500' };
  if (report?.valid) return { label: '校验通过', tone: 'bg-emerald-500' };
  if (report) return { label: `${report.issues.length} 个问题`, tone: 'bg-amber-500' };
  return { label: '就绪', tone: 'bg-emerald-500' };
}
