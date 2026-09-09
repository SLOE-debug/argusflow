use super::*;
use argusflow_core::OperationOptions;
#[test]
fn empty_map_is_success_and_rectangles_project_to_original_coordinates() {
    let mut values = vec![0.0; 64 * 32];
    let config = OcrConfig::new("unused");
    let operation = Operation::new(OperationOptions::default());
    assert!(
        boxes(
            &values,
            64,
            32,
            (128, 64),
            &config,
            &operation,
            Parameters {
                threshold: 0.2,
                box_threshold: 0.45,
                unclip: 1.4
            }
        )
        .unwrap()
        .is_empty()
    );
    for y in 8..20 {
        for x in 10..40 {
            values[y * 64 + x] = 0.9;
        }
    }
    let polygons = boxes(
        &values,
        64,
        32,
        (128, 64),
        &config,
        &operation,
        Parameters {
            threshold: 0.2,
            box_threshold: 0.45,
            unclip: 1.4,
        },
    )
    .unwrap();
    assert_eq!(polygons.len(), 1);
    assert!(
        polygons[0]
            .iter()
            .all(|p| p.x >= 0.0 && p.x < 128.0 && p.y >= 0.0 && p.y < 64.0)
    );
}
