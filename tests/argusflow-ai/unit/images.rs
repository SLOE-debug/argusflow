use crate::Evidence;
use argusflow_recorder::*;
use image::{DynamicImage, Rgb, RgbImage};
fn visual(attachment: Attachment, revision: u64) -> Visual {
    Visual {
        raw: vec![],
        relation: Relation::After,
        version: PixelVersion {
            session: "1".into(),
            source: 1,
            generation: 1,
            revision,
        },
        presented_ns: None,
        acquired_ns: revision,
        frozen_ns: revision,
        from_ns: revision,
        through_ns: revision,
        region: [0, 0, 100, 100],
        screen_origin: [0, 0],
        dpi: [96, 96],
        status: VisualStatus::Observed,
        decision: EvidenceDecision::VisualRequired("test".into()),
        image: Some(attachment),
    }
}
#[test]
fn change_images_are_scoped_and_bounded() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("attachments")).unwrap();
    let mut records = std::collections::BTreeMap::new();
    for revision in 1..=2 {
        let mut image = RgbImage::new(100, 100);
        if revision == 2 {
            image.put_pixel(40, 40, Rgb([255, 255, 255]));
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let attachment = publish_image(temp.path(), bytes.get_ref(), [100, 100]).unwrap();
        records.insert(
            revision,
            Record {
                id: revision,
                written_qpc: revision as i64,
                data: RecordData::Visual(visual(attachment, revision)),
            },
        );
    }
    let mut evidence = Evidence {
        directory: temp.path().into(),
        session: Session {
            id: "test".into(),
            format: 2,
            created_ms: 0,
            qpc_origin: 0,
            qpc_frequency: 1000,
            capture_session: None,
            policy: "test".into(),
        },
        records,
    };
    let (blocks, pixels) = evidence.change("2").unwrap();
    assert_eq!(blocks.len(), 4);
    assert!(pixels < 2000);
    assert!(evidence.change("../secret").is_err());
    if let RecordData::Visual(v) = &mut evidence.records.get_mut(&2).unwrap().data {
        v.version.generation = 2;
    }
    assert!(evidence.change("2").is_err());
}
