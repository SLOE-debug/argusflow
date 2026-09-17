import { useEffect, useState } from "react";
import { recorderApi } from "./api";
import type { Attachment } from "./model";
/** 一次只读取当前附件；切换后立即隐藏旧图并拒绝迟到结果。 */
export function useFrameImage(
  directory: string,
  attachment: Attachment | null,
) {
  const key = `${directory}:${attachment?.hash}`;
  const [state, setState] = useState({ key: "", url: "", error: "" });
  useEffect(() => {
    let cancelled = false;
    if (attachment)
      void recorderApi.image(directory, attachment).then(
        (url) => {
          if (!cancelled) setState({ key, url, error: "" });
        },
        (error) => {
          if (!cancelled) setState({ key, url: "", error: String(error) });
        },
      );
    return () => {
      cancelled = true;
    };
  }, [directory, key, attachment]);
  return state.key === key ? state : { key, url: "", error: "" };
}
