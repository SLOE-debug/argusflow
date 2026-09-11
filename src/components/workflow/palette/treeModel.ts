import { NODE_CATALOG, type NodeCategory } from "../../../features/workflow";

/** 常用是派生分组，其余分组与节点领域分类保持一致。 */
export const TREE_CATEGORIES: readonly (NodeCategory | "常用")[] = [
  "常用",
  "逻辑控制",
  "数据",
  "浏览器",
  "桌面自动化",
  "查询与操作",
];
export type TreeCategory = (typeof TREE_CATEGORIES)[number];
export type CatalogNode = (typeof NODE_CATALOG)[number];
/** 平面可见行显式声明层级，便于键盘移动和搜索展开。 */
export type TreeRow =
  | {
      readonly type: "category";
      readonly key: string;
      readonly category: TreeCategory;
      readonly expanded: boolean;
      readonly count: number;
    }
  | {
      readonly type: "node";
      readonly key: string;
      readonly category: TreeCategory;
      readonly node: CatalogNode;
      readonly position: number;
      readonly count: number;
    };

/** 搜索只临时展开匹配分类，不写入用户的折叠偏好。 */
export function visibleTreeRows(
  query: string,
  expanded: ReadonlySet<TreeCategory>,
  common: readonly CatalogNode[],
): readonly TreeRow[] {
  const search = query.trim().toLocaleLowerCase();
  return TREE_CATEGORIES.flatMap((category): TreeRow[] => {
    const nodes = (
      category === "常用"
        ? common
        : NODE_CATALOG.filter((item) => item.category === category)
    ).filter(
      (item) =>
        !search ||
        (item.title + " " + item.id + " " + item.description)
          .toLocaleLowerCase()
          .includes(search),
    );
    if (search && !nodes.length) return [];
    const open = Boolean(search) || expanded.has(category);
    return [
      {
        type: "category",
        key: category,
        category,
        expanded: open,
        count: nodes.length,
      },
      ...(open
        ? nodes.map((node, index): TreeRow => ({
            type: "node",
            key: category + ":" + node.id,
            category,
            node,
            position: index + 1,
            count: nodes.length,
          }))
        : []),
    ];
  });
}
