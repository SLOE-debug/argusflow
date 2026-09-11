import type { ReactNode } from "react";

/** 菜单项的展示与不可用状态，不依赖业务模型。 */
interface MenuItemPresentation {
  readonly label: string;
  readonly icon?: ReactNode;
  readonly disabled?: boolean;
  readonly reason?: string;
}

/** 动作、分组与级联入口互斥，子菜单不能同时执行动作。 */
export type MenuItem =
  | { readonly type: "separator"; readonly id: string }
  | (MenuItemPresentation & {
      readonly type: "action";
      readonly shortcut?: string;
      readonly danger?: boolean;
      readonly action: () => void;
    })
  | (MenuItemPresentation & {
      readonly type: "submenu";
      readonly items: readonly MenuItem[];
    });

/** 根菜单使用屏幕坐标，子菜单锚定触发项。 */
export type MenuAnchor =
  | { readonly type: "point"; readonly x: number; readonly y: number }
  | { readonly type: "submenu"; readonly trigger: HTMLElement };
