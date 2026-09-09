use super::*;
#[test]
fn noisy_probability_map_stops_at_candidate_budget() {
    let mut map = vec![0.0; 100 * 100];
    for y in (0..100).step_by(3) {
        for x in (0..100).step_by(3) {
            map[y * 100 + x] = 1.0;
        }
    }
    let op = Operation::new(argusflow_core::OperationOptions::default());
    assert_eq!(
        boundaries(&map, 100, 100, 0.2, 10, &op).unwrap_err().kind(),
        FailureKind::ResourceLimit
    );
}
