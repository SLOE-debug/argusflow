import type { AqlDiagnostic } from '../../workflow/contracts';
import { LineIndex } from '../core/LineIndex';

type DecorationLayerProps = Readonly<{
  source: string;
  diagnostics: readonly AqlDiagnostic[];
  scrollLeft: number;
  scrollTop: number;
}>;

/** 在独立层绘制诊断波浪线，避免修改输入 DOM 或 selection。 */
export function DecorationLayer({
  source,
  diagnostics,
  scrollLeft,
  scrollTop,
}: DecorationLayerProps) {
  const lineIndex = new LineIndex(source);
  const ranges = diagnostics
    .filter((diagnostic) => diagnostic.range !== null)
    .map((diagnostic) => ({
      diagnostic,
      offsets: lineIndex.toOffsets(diagnostic.range!),
    }))
    .sort((left, right) => left.offsets[0] - right.offsets[0]);
  let cursor = 0;

  return (
    <pre
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 m-0 min-h-[132px] min-w-full whitespace-pre p-3 font-mono text-[11px] leading-[18px] text-transparent"
      style={{ transform: `translate(${-scrollLeft}px, ${-scrollTop}px)` }}
    >
      {ranges.map(({ diagnostic, offsets }, index) => {
        const [rawStart, rawEnd] = offsets;
        const start = Math.max(cursor, rawStart);
        const end = Math.max(start + 1, rawEnd);
        const prefix = source.slice(cursor, start);
        const decorated = source.slice(start, end) || ' ';
        cursor = Math.max(cursor, end);
        return (
          <span key={`${diagnostic.code}-${index}`}>
            {prefix}
            <span
              className={diagnostic.severity === 'error'
                ? 'underline decoration-rose-500 decoration-wavy underline-offset-2'
                : 'underline decoration-amber-500 decoration-wavy underline-offset-2'}
            >
              {decorated}
            </span>
          </span>
        );
      })}
      {source.slice(cursor)}
      {'\n'}
    </pre>
  );
}
