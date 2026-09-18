//! 空间查询的编辑说明与执行语法共用关键字。
use super::{Symbol, SymbolKind};
pub(super) fn symbols() -> Vec<Symbol> {
    let example = "spatial(anchor=element(text=\"姓名\"),element=textbox(),direction=right,sort=near,select=第1个)";
    let mut symbols = vec![Symbol {
        name: "spatial".into(),
        kind: SymbolKind::Function,
        description: "在同一坐标空间内按锚点、角度、距离和对齐筛选；排序并列时拒绝猜测。".into(),
        signature: "spatial(anchor=查询,element=查询,条件)".into(),
        example: example.into(),
    }];
    for (name, description, signature) in [
        (
            "anchor",
            "必须唯一的锚点，可使用嵌套空间查询。",
            "anchor=element(text=\"姓名\")",
        ),
        (
            "direction",
            "八方向采用 45 度扇形，下界包含、上界排除。",
            "direction=right",
        ),
        (
            "angles",
            "向右为 0 度，逆时针增加；允许跨越 0 度。不能同时指定方向。",
            "angles=350 degrees_to 10 degrees",
        ),
        (
            "min_distance",
            "最小距离，包含边界。",
            "min_distance=10 logical_pixels",
        ),
        (
            "max_distance",
            "最大距离，按范围短边、宽度或高度的百分比换算。",
            "max_distance=scope_short 30%",
        ),
        (
            "distance_basis",
            "默认中心距离；边缘距离使用两个外框的最短距离。",
            "distance_basis=edge_distance",
        ),
        (
            "alignment",
            "按中心对齐，需要同时指定容差。",
            "alignment=same_row,tolerance=3 logical_pixels",
        ),
        (
            "tolerance",
            "同一行或列允许的中心偏差。",
            "tolerance=scope_height 2%",
        ),
        (
            "sort",
            "排序后选择第 N 个，缺失或并列都会报错。",
            "sort=near,select=第2个",
        ),
        (
            "secondary_sort",
            "主排序键相同时继续比较；仍相同则不确定。",
            "secondary_sort=top",
        ),
        (
            "select",
            "明确位置顺序；目标查询也可选择最左上角。",
            "select=第1个",
        ),
        (
            "scope",
            "以唯一目标的实际外框约束区域，并作为百分比长度基准。",
            "scope=element(text=\"面板\")",
        ),
        (
            "region_mode",
            "区域内要求完全包含，区域外要求没有正面积交集。",
            "region_mode=outside",
        ),
        (
            "exclude_overlap",
            "排除与锚点外框重叠的候选。",
            "exclude_overlap=true",
        ),
        (
            "second_anchor",
            "第二个唯一锚点，用于两点之间的查询。",
            "second_anchor=element(text=\"结束\")",
        ),
        (
            "between_mode",
            "线段模式必须指定带宽；矩形模式按两个锚点中心限定。",
            "between_mode=segment,bandwidth=10 logical_pixels",
        ),
        (
            "bandwidth",
            "线段两侧合计宽度，中心至线段距离不超过带宽的一半。",
            "bandwidth=scope_short 5%",
        ),
    ] {
        symbols.push(Symbol {
            name: name.into(),
            kind: SymbolKind::Attribute,
            description: description.into(),
            signature: signature.into(),
            example: example.into(),
        });
    }
    symbols
}
