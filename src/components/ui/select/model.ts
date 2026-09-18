import type { ReactNode } from "react";

/** 选项值直接传给业务方，不依赖原生 option 或伪造事件。 */
export interface SelectOption<T extends string> {
  /** 选项的稳定且唯一的值。 */
  readonly value: T;
  /** 同时用于触发器和菜单中的显示内容。 */
  readonly label: ReactNode;
  /** 禁用项仍显示，但不能被鼠标或键盘提交。 */
  readonly disabled?: boolean;
  /** 非文本标签可提供键盘搜索文本。 */
  readonly searchText?: string;
}

/** 面板以触发器的垂直中心展开，触及视口边缘时向另一侧平移。 */
export function selectPlacement(
  rect: Pick<DOMRect, "left" | "top" | "bottom" | "width">,
  viewport: { readonly width: number; readonly height: number },
  desiredHeight: number,
) {
  const margin = 8;
  const maxHeight = Math.min(
    desiredHeight,
    Math.max(0, viewport.height - margin * 2),
  );
  const width = Math.min(
    Math.max(rect.width, 120),
    Math.max(0, viewport.width - margin * 2),
  );
  return {
    left: Math.max(
      margin,
      Math.min(rect.left, viewport.width - width - margin),
    ),
    top: Math.max(
      margin,
      Math.min(
        (rect.top + rect.bottom - maxHeight) / 2,
        viewport.height - margin - maxHeight,
      ),
    ),
    width,
    maxHeight,
  };
}
