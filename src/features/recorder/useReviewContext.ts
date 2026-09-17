import { useEffect, useState } from "react";
import { recorderApi } from "./api";
import type { OperationContext } from "./model";

/** 选中事件独立读取证据，拖动合并短时间请求并拒绝迟到结果。 */
export function useReviewContext(directory: string, selected: number | null) {
  const [context, setContext] = useState<{
    readonly directory: string;
    readonly id: number;
    readonly data: OperationContext | null;
    readonly error: string;
  } | null>(null);
  useEffect(() => {
    if (selected === null) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      void recorderApi.context(directory, selected).then(
        (data) => {
          if (!cancelled)
            setContext({ directory, id: selected, data, error: "" });
        },
        (error) => {
          if (!cancelled)
            setContext({
              directory,
              id: selected,
              data: null,
              error: String(error),
            });
        },
      );
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [directory, selected]);
  return context?.directory === directory && context.id === selected
    ? context
    : null;
}
