//! 纯像素、查询和恢复策略测试，不访问或操作真实桌面。
#![allow(dead_code, reason = "共享 demo 模块还提供真实桌面流程使用的入口")]
#[path = "../support/wechat_demo/conversation_state.rs"]
mod conversation_state;
#[path = "../support/wechat_demo/frame.rs"]
mod frame;
#[path = "../support/wechat_demo/incremental.rs"]
mod incremental;
#[path = "../support/wechat_demo/layout.rs"]
mod layout;
#[path = "../support/wechat_demo/recovery.rs"]
mod recovery;
#[path = "../support/wechat_demo/snapshot.rs"]
mod snapshot;
#[path = "../support/wechat_demo/spatial.rs"]
mod spatial;
#[path = "../support/wechat_demo/text.rs"]
mod text;
use argusflow_capture_contracts::{PixelRect, ScreenRect};
use argusflow_core::{ImagePoint, Operation, OperationOptions};
use frame::Frame;
use incremental::{UpdateKind, merge, plan};
use recovery::{ConversationState as State, Decision, decide};
use std::sync::Arc;
use text::TextRecord;

fn rectangle(x: u32, y: u32, w: u32, h: u32) -> PixelRect {
    PixelRect::new(x, y, w, h).unwrap()
}
fn blank() -> Frame {
    Frame::from_bgrx(
        ScreenRect::new(-1000, 50, 800, 600).unwrap(),
        vec![0; 800 * 600 * 4],
    )
    .unwrap()
}
fn paint(frame: &mut Frame, rect: PixelRect, value: u8) {
    let pixels = Arc::make_mut(&mut frame.pixels);
    for y in rect.y()..rect.bottom() {
        for x in rect.x()..rect.right() {
            let offset = (y as usize * frame.bounds.width() as usize + x as usize) * 3;
            pixels[offset..offset + 3].fill(value);
        }
    }
}
fn block(text: &str, rect: PixelRect) -> TextRecord {
    TextRecord {
        text: text.into(),
        confidence: 0.99,
        rect,
        polygon: [
            ImagePoint {
                x: rect.x() as f32,
                y: rect.y() as f32,
            },
            ImagePoint {
                x: rect.right() as f32,
                y: rect.y() as f32,
            },
            ImagePoint {
                x: rect.right() as f32,
                y: rect.bottom() as f32,
            },
            ImagePoint {
                x: rect.x() as f32,
                y: rect.bottom() as f32,
            },
        ],
    }
}
fn operation() -> Operation {
    Operation::new(OperationOptions::default())
}
#[test]
fn first_frame_full_then_unchanged_and_moved_frame_skip_models() {
    let before = blank();
    assert_eq!(
        plan(None, &before, &[], &operation()).unwrap().kind,
        UpdateKind::Initial
    );
    let mut after = before.clone();
    after.bounds = ScreenRect::new(200, 100, 800, 600).unwrap();
    let update = plan(Some(&before), &after, &[], &operation()).unwrap();
    assert_eq!(update.kind, UpdateKind::Unchanged);
    assert!(update.regions.is_empty());
}
#[test]
fn resized_frame_discards_all_old_geometry() {
    let before = blank();
    let after = Frame::from_bgrx(
        ScreenRect::new(0, 0, 400, 300).unwrap(),
        vec![0; 400 * 300 * 4],
    )
    .unwrap();
    let update = plan(Some(&before), &after, &[], &operation()).unwrap();
    assert_eq!(update.kind, UpdateKind::Resized);
    assert!(
        merge(
            &[block("旧标题", rectangle(600, 10, 80, 20))],
            &update,
            vec![]
        )
        .is_empty()
    );
}
#[test]
fn blanked_title_is_removed_without_dropping_unchanged_sidebar() {
    let title = rectangle(450, 40, 100, 20);
    let mut before = blank();
    paint(&mut before, title, 255);
    let after = blank();
    let old = vec![
        block("搜索", rectangle(80, 40, 60, 20)),
        block("文件传输助手", title),
    ];
    let update = plan(Some(&before), &after, &old, &operation()).unwrap();
    assert_eq!(update.kind, UpdateKind::Incremental);
    assert!(update.regions.iter().any(|r| r.contains(title)));
    let blocks = merge(&old, &update, vec![]);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text, "搜索");
}
#[test]
fn partial_character_change_expands_over_entire_cached_line() {
    let before = blank();
    let mut after = before.clone();
    paint(&mut after, rectangle(350, 100, 3, 12), 255);
    let line = rectangle(200, 95, 300, 25);
    let update = plan(
        Some(&before),
        &after,
        &[block("旧的长行文字", line)],
        &operation(),
    )
    .unwrap();
    assert!(update.regions.iter().any(|r| r.contains(line)));
    assert_eq!(update.kind, UpdateKind::Incremental);
}
#[test]
fn disconnected_updates_remain_small_nonoverlapping_crops() {
    let before = blank();
    let mut after = before.clone();
    paint(&mut after, rectangle(10, 10, 15, 15), 255);
    paint(&mut after, rectangle(700, 500, 15, 15), 255);
    let update = plan(Some(&before), &after, &[], &operation()).unwrap();
    assert_eq!(update.regions.len(), 2);
    assert!(update.regions[0].intersection(update.regions[1]).is_none());
    assert!(
        update
            .regions
            .iter()
            .all(|r| after.bounds.local().contains(*r))
    );
}
#[test]
fn broad_changes_explicitly_rebuild_full_snapshot() {
    let before = blank();
    let mut after = before.clone();
    paint(&mut after, rectangle(0, 0, 800, 500), 255);
    assert_eq!(
        plan(Some(&before), &after, &[], &operation()).unwrap().kind,
        UpdateKind::BroadChange
    );
}
#[test]
fn filler_channel_changes_are_not_visual_changes() {
    let bounds = ScreenRect::new(0, 0, 1, 1).unwrap();
    let a = Frame::from_bgrx(bounds, vec![1, 2, 3, 0]).unwrap();
    let b = Frame::from_bgrx(bounds, vec![1, 2, 3, 255]).unwrap();
    assert_eq!(a.pixels, b.pixels);
    assert!(Frame::from_bgrx(bounds, vec![0; 3]).is_err());
    assert!(a.crop(rectangle(1, 0, 1, 1)).is_err());
}
#[test]
fn aql_region_filter_preserves_original_block_index_and_screen_origin() {
    let snapshot = snapshot::Observation {
        frame: blank(),
        blocks: Arc::new(vec![
            block("无关文字", rectangle(10, 10, 50, 20)),
            block("文件传输助手", rectangle(450, 40, 100, 20)),
        ]),
    };
    let matches = snapshot
        .text(rectangle(400, 0, 400, 100), "文件传输助手", false)
        .unwrap();
    assert_eq!(matches[0].index, 1);
    assert_eq!(matches[0].point.x, -500);
    assert_eq!(matches[0].point.y, 100);
    assert!(
        snapshot
            .text(rectangle(0, 0, 300, 100), "文件传输助手", false)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn recovery_never_clicks_an_open_conversation_and_has_one_blank_retry() {
    for clicks in 0..=3 {
        assert_eq!(decide(State::Ready, clicks), Decision::Keep);
    }
    assert_eq!(decide(State::Other, 0), Decision::Open);
    assert_eq!(decide(State::Empty, 0), Decision::Open);
    assert_eq!(decide(State::Empty, 1), Decision::Recover);
    assert_eq!(decide(State::Empty, 2), Decision::Stop);
    assert_eq!(decide(State::Other, 1), Decision::Stop);
}

#[test]
fn blank_page_ignores_window_controls_but_requires_search_anchor() {
    let records = vec![
        block("搜索", rectangle(90, 40, 40, 20)),
        block("×", rectangle(770, 10, 20, 15)),
    ];
    let blank = snapshot::Observation {
        frame: blank(),
        blocks: Arc::new(records.clone()),
    };
    assert_eq!(conversation_state::state(&blank).unwrap(), State::Empty);
    let mut ready = blank.clone();
    Arc::make_mut(&mut ready.blocks).push(block("文件传输助手", rectangle(450, 40, 100, 20)));
    assert_eq!(conversation_state::state(&ready).unwrap(), State::Ready);
    let mut other = blank.clone();
    Arc::make_mut(&mut other.blocks).push(block("其他联系人", rectangle(450, 40, 100, 20)));
    assert_eq!(conversation_state::state(&other).unwrap(), State::Other);
    let mut unknown = blank;
    Arc::make_mut(&mut unknown.blocks).clear();
    assert!(conversation_state::state(&unknown).is_err());
}

#[test]
fn blank_then_ready_sequence_retries_navigation_only_once() {
    let states = [State::Other, State::Empty, State::Ready];
    let mut clicks = 0;
    let actions: Vec<_> = states
        .into_iter()
        .map(|state| {
            let action = decide(state, clicks);
            if matches!(action, Decision::Open | Decision::Recover) {
                clicks += 1;
            }
            action
        })
        .collect();
    assert_eq!(
        actions,
        vec![Decision::Open, Decision::Recover, Decision::Keep]
    );
    assert_eq!(clicks, 2);
}
