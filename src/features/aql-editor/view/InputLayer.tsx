import type {
  ChangeEvent,
  CompositionEvent,
  KeyboardEvent,
  RefObject,
  UIEvent,
} from 'react';

type InputLayerProps = Readonly<{
  inputRef: RefObject<HTMLTextAreaElement | null>;
  source: string;
  invalid: boolean;
  highlighted: boolean;
  onChange: (event: ChangeEvent<HTMLTextAreaElement>) => void;
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  onSelect: () => void;
  onScroll: (event: UIEvent<HTMLTextAreaElement>) => void;
  onCompositionStart: (event: CompositionEvent<HTMLTextAreaElement>) => void;
  onCompositionEnd: (event: CompositionEvent<HTMLTextAreaElement>) => void;
}>;

/** 只负责原生 IME、caret、selection、clipboard 与系统输入事件的 textarea。 */
export function InputLayer({
  inputRef,
  source,
  invalid,
  highlighted,
  onChange,
  onKeyDown,
  onSelect,
  onScroll,
  onCompositionStart,
  onCompositionEnd,
}: InputLayerProps) {
  return (
    <textarea
      ref={inputRef}
      aria-label="AQL 查询"
      aria-invalid={invalid}
      className={
        'absolute inset-0 z-20 h-full w-full resize-none overflow-auto whitespace-pre border-0 bg-transparent p-3 font-mono text-[11px] leading-[18px] caret-slate-800 outline-none selection:bg-blue-200/70 ' +
        (highlighted ? 'text-transparent' : 'text-slate-700')
      }
      spellCheck={false}
      wrap="off"
      value={source}
      onChange={onChange}
      onKeyDown={onKeyDown}
      onSelect={onSelect}
      onScroll={onScroll}
      onCompositionStart={onCompositionStart}
      onCompositionEnd={onCompositionEnd}
    />
  );
}
