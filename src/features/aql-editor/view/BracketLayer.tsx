import type { EditorRange } from '../../workflow/contracts';
import { LineIndex } from '../core/LineIndex';

type BracketLayerProps = Readonly<{
  source: string;
  ranges: readonly EditorRange[];
  scrollLeft: number;
  scrollTop: number;
}>;

/** 使用 Rust CST 返回的范围标记活动括号对。 */
export function BracketLayer({
  source,
  ranges,
  scrollLeft,
  scrollTop,
}: BracketLayerProps) {
  const lineIndex = new LineIndex(source);
  const offsets = ranges
    .map((range) => lineIndex.toOffsets(range))
    .sort((left, right) => left[0] - right[0]);
  let cursor = 0;

  return (
    <pre
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 m-0 min-h-[132px] min-w-full whitespace-pre p-3 font-mono text-[11px] leading-[18px] text-transparent"
      style={{ transform: `translate(${-scrollLeft}px, ${-scrollTop}px)` }}
    >
      {offsets.map(([start, end], index) => {
        const prefix = source.slice(cursor, start);
        const bracket = source.slice(start, end);
        cursor = end;
        return (
          <span key={`${start}-${end}-${index}`}>
            {prefix}
            <span className="rounded-sm bg-blue-200/80 ring-1 ring-blue-400/60">
              {bracket}
            </span>
          </span>
        );
      })}
      {source.slice(cursor)}
      {'\n'}
    </pre>
  );
}
