//! 不返回原生句柄；每个快照明确区分来源与坐标空间。
use argusflow_aql::{Attribute, Value as AqlValue};
use argusflow_automation::LocatedElement;
use argusflow_runtime::RunError;
use argusflow_workflow::{Value as V, ValueType as T};
use std::collections::BTreeMap;

pub(super) fn match_type() -> T {
    T::Record(
        [
            ("source".into(), T::Text),
            ("space".into(), T::Text),
            ("frame".into(), T::Optional(Box::new(T::Text))),
            ("name".into(), T::Optional(Box::new(T::Text))),
            ("text".into(), T::Optional(Box::new(T::Text))),
            ("confidence".into(), T::Optional(Box::new(T::Float))),
            ("bounds".into(), T::List(Box::new(T::Float))),
        ]
        .into(),
    )
}
fn optional_text(value: Option<&str>) -> V {
    V::Optional(value.map(|s| Box::new(V::Text(s.into()))))
}
fn attribute<'a>(fields: &'a BTreeMap<Attribute, AqlValue>, key: &Attribute) -> Option<&'a str> {
    match fields.get(key) {
        Some(AqlValue::Text(value)) => Some(value),
        _ => None,
    }
}
pub(super) fn snapshot(element: &LocatedElement) -> Result<V, RunError> {
    let mut fields = BTreeMap::from([
        ("frame".into(), optional_text(None)),
        ("name".into(), optional_text(None)),
        ("text".into(), optional_text(None)),
        ("confidence".into(), V::Optional(None)),
    ]);
    let (source, space, bounds) = match element {
        LocatedElement::Browser(element) => {
            fields.insert("frame".into(), optional_text(Some(element.frame_id())));
            fields.insert(
                "name".into(),
                optional_text(attribute(element.attributes(), &Attribute::Name)),
            );
            fields.insert(
                "text".into(),
                optional_text(attribute(element.attributes(), &Attribute::Text)),
            );
            let [x, y, width, height] = element.bounds();
            ("dom", "frame_css_pixels", [x, y, x + width, y + height])
        }
        #[cfg(windows)]
        LocatedElement::Uia(element) => {
            fields.insert("name".into(), optional_text(Some(&element.snapshot().name)));
            fields.insert(
                "text".into(),
                optional_text(attribute(element.attributes(), &Attribute::Text)),
            );
            (
                "uia",
                "screen_physical_pixels",
                element.snapshot().bounds.map(f64::from),
            )
        }
        #[cfg(windows)]
        LocatedElement::Ocr { sample, target } => {
            fields.insert("text".into(), optional_text(Some(target.text())));
            fields.insert(
                "confidence".into(),
                V::Optional(Some(Box::new(V::Float(f64::from(target.confidence()))))),
            );
            let polygon = sample.screen_polygon(target.index()).ok_or_else(|| {
                RunError::new(
                    argusflow_workflow::ErrorKind::Contract,
                    "OCR 快照缺少对应几何",
                )
            })?;
            let left = polygon.iter().map(|p| p.x).min().unwrap_or(0);
            let top = polygon.iter().map(|p| p.y).min().unwrap_or(0);
            let right = polygon.iter().map(|p| p.x).max().unwrap_or(0);
            let bottom = polygon.iter().map(|p| p.y).max().unwrap_or(0);
            (
                "ocr",
                "screen_physical_pixels",
                [left, top, right, bottom].map(f64::from),
            )
        }
    };
    fields.insert("source".into(), V::Text(source.into()));
    fields.insert("space".into(), V::Text(space.into()));
    fields.insert(
        "bounds".into(),
        V::List(bounds.into_iter().map(V::Float).collect()),
    );
    Ok(V::Record(fields))
}
