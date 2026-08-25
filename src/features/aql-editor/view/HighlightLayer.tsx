import { Fragment } from 'react';

import { LineIndex } from '../core/LineIndex';
import type { SyntaxToken, SyntaxTokenKind } from '../language/types';

const TOKEN_CLASS_NAMES: Readonly<Record<SyntaxTokenKind, string>> = {
  role: 'font-semibold text-violet-700',
  function: 'font-semibold text-blue-700',
  property: 'text-cyan-700',
  namespace: 'text-amber-700 underline decoration-dotted underline-offset-2',
  operator: 'font-semibold text-fuchsia-700',
  string: 'text-emerald-700',
  regex: 'text-orange-700',
  boolean: 'text-rose-700',
  integer: 'text-rose-700',
  punctuation: 'text-slate-500',
  trivia: 'text-slate-700',
  unknown: 'text-slate-700',
};

type HighlightLayerProps = Readonly<{
  source: string;
  tokens: readonly SyntaxToken[];
  scrollLeft: number;
  scrollTop: number;
}>;

/** 使用 Rust semantic tokens 渲染的只读高亮层。 */
export function HighlightLayer({
  source,
  tokens,
  scrollLeft,
  scrollTop,
}: HighlightLayerProps) {
  const lineIndex = new LineIndex(source);
  let cursor = 0;

  return (
    <pre
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 m-0 min-h-[132px] min-w-full whitespace-pre p-3 font-mono text-[11px] leading-[18px]"
      style={{ transform: `translate(${-scrollLeft}px, ${-scrollTop}px)` }}
    >
      {tokens.map((token, index) => {
        const [start, end] = lineIndex.toOffsets(token.range);
        const gap = start > cursor ? source.slice(cursor, start) : '';
        const text = source.slice(start, end);
        cursor = Math.max(cursor, end);
        return (
          <Fragment key={`${index}-${start}-${end}`}>
            {gap}
            <span className={TOKEN_CLASS_NAMES[token.kind]}>{text}</span>
          </Fragment>
        );
      })}
      {source.slice(cursor)}
      {'\n'}
    </pre>
  );
}
