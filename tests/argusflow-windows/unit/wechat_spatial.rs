//! Demo 空间关系与排序契约，使用合成坐标，不操作桌面。
#[path = "../support/wechat_demo/spatial.rs"]
mod spatial;
use argusflow_core::ScreenPoint;
use spatial::{Candidate, Direction, nearest, top_left};
use std::num::NonZeroUsize;

fn candidate(index: usize, x: i32, y: i32) -> Candidate {
    Candidate {
        text: "目标".into(),
        point: ScreenPoint { x, y },
        index,
    }
}
#[test]
fn direction_alignment_and_second_nearest() {
    let anchor = candidate(0, 0, 0);
    let candidates = vec![
        candidate(1, 10, 0),
        candidate(2, 20, 0),
        candidate(3, 1, 20),
        candidate(4, -1, 0),
    ];
    assert_eq!(
        nearest(&candidates, &anchor, Direction::Right, NonZeroUsize::MIN, 5)
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        nearest(
            &candidates,
            &anchor,
            Direction::Right,
            NonZeroUsize::new(2).unwrap(),
            5
        )
        .unwrap()
        .index,
        2
    );
    assert_eq!(
        nearest(&candidates, &anchor, Direction::Left, NonZeroUsize::MIN, 5)
            .unwrap()
            .index,
        4
    );
    assert_eq!(
        nearest(&candidates, &anchor, Direction::Below, NonZeroUsize::MIN, 5)
            .unwrap()
            .index,
        3
    );
    assert!(nearest(&candidates, &anchor, Direction::Above, NonZeroUsize::MIN, 5).is_none());
}
#[test]
fn equal_distance_and_negative_monitor_origin_are_deterministic() {
    let anchor = candidate(0, -200, -100);
    let candidates = vec![candidate(2, -190, -99), candidate(1, -190, -101)];
    assert_eq!(
        nearest(&candidates, &anchor, Direction::Right, NonZeroUsize::MIN, 5)
            .unwrap()
            .index,
        1
    );
    assert_eq!(top_left(&candidates).unwrap().index, 1);
    assert!(
        nearest(
            &candidates,
            &anchor,
            Direction::Right,
            NonZeroUsize::new(3).unwrap(),
            5
        )
        .is_none()
    );
}
#[test]
fn full_coordinate_range_does_not_overflow() {
    let anchor = candidate(0, i32::MIN, i32::MIN);
    let candidates = vec![candidate(1, i32::MAX, i32::MAX)];
    assert!(
        nearest(
            &candidates,
            &anchor,
            Direction::Right,
            NonZeroUsize::MIN,
            u32::MAX
        )
        .is_some()
    );
}
