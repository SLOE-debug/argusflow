//! 空间预览的工作流值契约，不依赖输出变量名或 JSON 文本约定。
use argusflow_aql::SpatialPreview;
use argusflow_workflow::{Value as V, ValueType as T};

pub(super) fn value_type() -> T {
    let floats = T::List(Box::new(T::Float));
    let candidate = T::Record(
        [
            ("node".into(), T::Int),
            ("bounds".into(), floats.clone()),
            ("angle".into(), T::Optional(Box::new(T::Float))),
            ("distance".into(), T::Float),
            ("reason".into(), T::Optional(Box::new(T::Text))),
            ("rank".into(), T::Optional(Box::new(T::Int))),
            ("selected".into(), T::Bool),
        ]
        .into(),
    );
    let item = T::Record(
        [
            ("space".into(), T::Text),
            ("scope".into(), floats.clone()),
            ("anchor".into(), floats.clone()),
            ("angles".into(), T::Optional(Box::new(floats))),
            ("candidates".into(), T::List(Box::new(candidate))),
        ]
        .into(),
    );
    T::Record(
        [
            ("kind".into(), T::Text),
            ("items".into(), T::List(Box::new(item))),
        ]
        .into(),
    )
}
fn floats(values: impl IntoIterator<Item = f64>) -> V {
    V::List(values.into_iter().map(V::Float).collect())
}
fn optional(value: Option<V>) -> V {
    V::Optional(value.map(Box::new))
}
pub(super) fn value(previews: Vec<SpatialPreview>) -> V {
    let items = previews
        .into_iter()
        .map(|p| {
            V::Record(
                [
                    ("space".into(), V::Text(p.space)),
                    ("scope".into(), floats(p.scope.coordinates())),
                    ("anchor".into(), floats(p.anchor.coordinates())),
                    (
                        "angles".into(),
                        optional(p.angles.map(|(a, b)| floats([a, b]))),
                    ),
                    (
                        "candidates".into(),
                        V::List(
                            p.candidates
                                .into_iter()
                                .map(|c| {
                                    V::Record(
                                        [
                                            ("node".into(), V::Int(c.node as i64)),
                                            ("bounds".into(), floats(c.bounds.coordinates())),
                                            ("angle".into(), optional(c.angle.map(V::Float))),
                                            ("distance".into(), V::Float(c.distance)),
                                            ("reason".into(), optional(c.reason.map(V::Text))),
                                            (
                                                "rank".into(),
                                                optional(c.rank.map(|r| V::Int(r as i64))),
                                            ),
                                            ("selected".into(), V::Bool(c.selected)),
                                        ]
                                        .into(),
                                    )
                                })
                                .collect(),
                        ),
                    ),
                ]
                .into(),
            )
        })
        .collect();
    V::Record(
        [
            ("kind".into(), V::Text("spatial_preview".into())),
            ("items".into(), V::List(items)),
        ]
        .into(),
    )
}
