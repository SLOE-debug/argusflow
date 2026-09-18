use super::*;
use crate::{Attribute, Bindings, Node, Role, compile_target, evaluate, preview};
use std::collections::BTreeMap;
fn tree(points: &[(&str, f64, f64)]) -> QueryTree<()> {
    let mut tree = QueryTree::new(100, 8, 100).unwrap();
    for &(text, x, y) in points {
        tree.push(
            Node::element(
                None,
                Role::Text,
                BTreeMap::from([(Attribute::Text, Value::Text(text.into()))]),
                (),
            )
            .with_geometry(
                Geometry::new(
                    Rect::new([x - 1., y - 1., 2., 2.]).unwrap(),
                    Rect::new([-500., -500., 1000., 1000.]).unwrap(),
                    "fixture",
                    Some(2.),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    }
    tree
}
fn query(source: &str) -> crate::BoundQuery {
    compile_target(source)
        .unwrap()
        .bind(&Bindings::new())
        .unwrap()
}
#[test]
fn eight_directions_and_wrapped_angles() {
    let points = [
        ("锚", 0., 0.),
        ("右", 100., 0.),
        ("上右", 100., -100.),
        ("上", 0., -100.),
        ("上左", -100., -100.),
        ("左", -100., 0.),
        ("下左", -100., 100.),
        ("下", 0., 100.),
        ("下右", 100., 100.),
    ];
    let tree = tree(&points);
    for (index, (direction, _, _)) in points.iter().enumerate().skip(1) {
        let q = query(&format!(
            "空间查找(锚点=目标(文本=\"锚\"),目标=目标(),方向={direction})"
        ));
        assert_eq!(
            evaluate(&q, &tree, &Operation::unbounded()).unwrap(),
            vec![index]
        );
    }
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(),角度区间=350度到10度)");
    assert_eq!(evaluate(&q, &tree, &Operation::unbounded()).unwrap(), [1]);
}
#[test]
fn bound_lengths_alignment_and_preview() {
    let tree = tree(&[
        ("锚", 0., 0.),
        ("候选", 40., 0.),
        ("候选", 80., 1.),
        ("候选", 60., 30.),
    ]);
    let q=compile_target("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),方向=右,最大距离=范围短边的$距离%,对齐=同一行,对齐容差=2逻辑像素,排序=距离从近到远,选择=第2个)").unwrap().bind(&Bindings::from([("距离".into(),Value::Number(10.))])).unwrap();
    assert_eq!(evaluate(&q, &tree, &Operation::unbounded()).unwrap(), [2]);
    let report = preview(&q, &tree, &Operation::unbounded()).unwrap();
    assert!(
        report[0]
            .candidates
            .iter()
            .any(|c| c.selected && c.node == 2 && c.rank == Some(2))
    );
    assert!(report[0].candidates.iter().any(|c| c.reason.is_some()));
}
#[test]
fn ties_rank_and_duplicate_anchors_fail() {
    let tree = tree(&[("锚", 0., 0.), ("候选", 10., -1.), ("候选", 10., 1.)]);
    for (suffix, kind) in [
        ("排序=距离从近到远,选择=第1个", FailureKind::Ambiguous),
        ("排序=距离从近到远,选择=第3个", FailureKind::NotFound),
    ] {
        let q = query(&format!(
            "空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),{suffix})"
        ));
        assert_eq!(
            evaluate(&q, &tree, &Operation::unbounded())
                .unwrap_err()
                .kind(),
            kind
        );
    }
    let q = query(
        "空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),排序=距离从近到远,次排序=从上到下,选择=第1个)",
    );
    assert_eq!(evaluate(&q, &tree, &Operation::unbounded()).unwrap(), [1]);
    let q = query("空间查找(锚点=目标(文本=\"候选\"),目标=目标())");
    assert_eq!(
        evaluate(&q, &tree, &Operation::unbounded())
            .unwrap_err()
            .kind(),
        FailureKind::Ambiguous
    );
}
#[test]
fn compound_queries_and_invalid_options() {
    for source in [
        "空间查找(锚点=目标(文本=\"搜索\",选择=最左上角),目标=输入框(),方向=右,距离依据=边缘距离,最大距离=范围短边的30%,排序=距离从近到远,选择=第2个)",
        "空间查找(锚点=空间查找(锚点=目标(文本=\"菜单\"),目标=目标(文本=\"设置\"),方向=下),目标=目标(文本=\"保存\"),方向=右)",
        "空间查找(锚点=目标(文本=\"起始\"),第二锚点=目标(文本=\"结束\"),目标=输入框(),两锚点模式=线段,带宽=范围高度的2%)",
    ] {
        compile_target(source).unwrap();
    }
    for source in [
        "空间查找(锚点=目标(),目标=目标(),方向=右,角度区间=25度到45度)",
        "空间查找(锚点=目标(),目标=目标(),角度区间=0度到0度)",
        "空间查找(锚点=目标(),目标=目标(),选择=第0个)",
        "空间查找(锚点=目标(),目标=目标(),对齐=同一行)",
    ] {
        assert!(compile_target(source).is_err(), "{source}");
    }
}

#[test]
fn edge_distance_nested_filters_and_segment_band() {
    let t = tree(&[
        ("锚", 0., 0.),
        ("结束", 100., 0.),
        ("候选", 50., 3.),
        ("候选", 50., 20.),
        ("候选", 120., 0.),
    ]);
    let q = query(
        "空间查找(锚点=目标(文本=\"锚\"),第二锚点=目标(文本=\"结束\"),目标=目标(文本=\"候选\"),两锚点模式=线段,带宽=4逻辑像素)",
    );
    assert_eq!(evaluate(&q, &t, &Operation::unbounded()).unwrap(), [2]);
    let q = query(
        "空间查找(锚点=空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),方向=右,排序=距离从近到远,选择=第1个),目标=目标(文本=\"结束\"),方向=右)",
    );
    assert_eq!(evaluate(&q, &t, &Operation::unbounded()).unwrap(), [1]);
    let t = tree(&[("锚", 0., 0.), ("候选", 10., 0.)]);
    let q = query(
        "空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),距离依据=边缘距离,最大距离=4逻辑像素)",
    );
    assert_eq!(evaluate(&q, &t, &Operation::unbounded()).unwrap(), [1]);
    assert_eq!(
        preview(&q, &t, &Operation::unbounded()).unwrap()[0].candidates[0].distance,
        8.
    );
}

#[test]
fn missing_or_cross_space_geometry_is_rejected() {
    let mut t = tree(&[("锚", 0., 0.)]);
    t.push(Node::element(
        None,
        Role::Text,
        [(Attribute::Text, Value::Text("候选".into()))].into(),
        (),
    ))
    .unwrap();
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),方向=右)");
    assert_eq!(
        evaluate(&q, &t, &Operation::unbounded())
            .unwrap_err()
            .kind(),
        FailureKind::Unsupported
    );
    let mut t = tree(&[("锚", 0., 0.)]);
    t.push(
        Node::element(
            None,
            Role::Text,
            [(Attribute::Text, Value::Text("候选".into()))].into(),
            (),
        )
        .with_geometry(
            Geometry::new(
                Rect::new([10., 0., 2., 2.]).unwrap(),
                Rect::new([-500., -500., 1000., 1000.]).unwrap(),
                "another_frame",
                Some(2.),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        evaluate(&q, &t, &Operation::unbounded())
            .unwrap_err()
            .kind(),
        FailureKind::Unsupported
    );
}

#[test]
fn coincident_centers_do_not_have_a_direction() {
    let t = tree(&[("锚", 0., 0.), ("候选", 0., 0.)]);
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),方向=右)");
    assert!(
        evaluate(&q, &t, &Operation::unbounded())
            .unwrap()
            .is_empty()
    );
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),排除重叠=是)");
    assert!(
        evaluate(&q, &t, &Operation::unbounded())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn angle_lower_is_inclusive_upper_exclusive() {
    let t = tree(&[
        ("锚", 0., 0.),
        ("候选", 10., 0.),
        ("候选", 0., -10.),
        ("候选", 10., -10.),
    ]);
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),角度区间=0度到90度)");
    assert_eq!(evaluate(&q, &t, &Operation::unbounded()).unwrap(), [1, 3]);
}

#[test]
fn explicit_regions_and_unknown_dpi_have_strict_semantics() {
    let mut t = QueryTree::new(10, 2, 10).unwrap();
    for (name, bounds) in [
        ("锚", [0., 0., 2., 2.]),
        ("区域", [10., 10., 40., 40.]),
        ("候选", [20., 20., 2., 2.]),
        ("候选", [60., 20., 2., 2.]),
    ] {
        t.push(
            Node::element(
                None,
                Role::Text,
                [(Attribute::Text, Value::Text(name.into()))].into(),
                (),
            )
            .with_geometry(
                Geometry::new(
                    Rect::new(bounds).unwrap(),
                    Rect::new([0., 0., 100., 100.]).unwrap(),
                    "image",
                    None,
                )
                .unwrap(),
            ),
        )
        .unwrap();
    }
    for (mode, expected) in [("区域内", 2), ("区域外", 3)] {
        let q = query(&format!(
            "空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),范围=目标(文本=\"区域\"),区域关系={mode})"
        ));
        assert_eq!(
            evaluate(&q, &t, &Operation::unbounded()).unwrap(),
            [expected]
        );
    }
    let q = query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),最大距离=100逻辑像素)");
    assert_eq!(
        evaluate(&q, &t, &Operation::unbounded())
            .unwrap_err()
            .kind(),
        FailureKind::Unsupported
    );
    let q =
        query("空间查找(锚点=目标(文本=\"锚\"),目标=目标(文本=\"候选\"),最大距离=范围短边的100%)");
    assert_eq!(evaluate(&q, &t, &Operation::unbounded()).unwrap(), [2, 3]);
}
