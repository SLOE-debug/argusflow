import type { AqlDiagnostic } from '../../workflow/contracts';

type GutterProps = Readonly<{
  lineCount: number;
  diagnostics: readonly AqlDiagnostic[];
  scrollTop: number;
}>;

/** 行号与诊断标记 gutter。 */
export function Gutter({ lineCount, diagnostics, scrollTop }: GutterProps) {
  const diagnosticLines = new Set(
    diagnostics.flatMap((diagnostic) => diagnostic.range ? [diagnostic.range.start.line] : []),
  );

  return (
    <div className="relative w-9 shrink-0 overflow-hidden border-r border-slate-200 bg-slate-50">
      <div
        className="absolute inset-x-0 top-0 py-3 font-mono text-[9px] leading-[18px] text-slate-400"
        style={{ transform: `translateY(${-scrollTop}px)` }}
      >
        {Array.from({ length: lineCount }, (_, line) => (
          <div key={line} className="flex h-[18px] items-center justify-end gap-1 pr-1.5">
            {diagnosticLines.has(line) ? (
              <span className="size-1 rounded-full bg-rose-500" aria-hidden="true" />
            ) : null}
            <span>{line + 1}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
