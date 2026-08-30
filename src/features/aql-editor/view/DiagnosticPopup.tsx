import AlertTriangle from 'lucide-react/dist/esm/icons/triangle-alert.mjs';

import type { AqlDiagnostic } from '../../workflow';
import { diagnosticLabel } from '../language/messages';

/** 展示第一个高优先级诊断；完整列表仍保留在 decoration 与 Explain 中。 */
export function DiagnosticPopup({
  diagnostics,
}: Readonly<{ diagnostics: readonly AqlDiagnostic[] }>) {
  const diagnostic = diagnostics.find((candidate) => candidate.severity === 'error')
    ?? diagnostics[0];
  if (!diagnostic) {
    return null;
  }
  const position = diagnostic.range?.start;

  return (
    <div
      className={diagnostic.severity === 'error'
        ? 'flex items-start gap-1.5 rounded-md border border-rose-200 bg-rose-50 px-2.5 py-2 text-[10px] leading-4 text-rose-700'
        : 'flex items-start gap-1.5 rounded-md border border-amber-200 bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700'}
      role={diagnostic.severity === 'error' ? 'alert' : 'status'}
    >
      <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
      <span>
        {position ? `第 ${position.line + 1} 行，第 ${position.utf16_column + 1} 列：` : ''}
        {diagnosticLabel(diagnostic)}
      </span>
    </div>
  );
}
