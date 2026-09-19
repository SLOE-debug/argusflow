//! 重复消息的轨迹和确认边界；合成输入不操作真实微信。
#![allow(dead_code, reason = "共享 demo 模块包含真实 UI 使用的入口")]
#[path = "../support/wechat_demo/bubbles.rs"]
mod bubbles;
#[path = "../support/wechat_demo/conversation_state.rs"]
mod conversation_state;
#[path = "../support/wechat_demo/editor.rs"]
mod editor;
#[path = "../support/wechat_demo/frame.rs"]
mod frame;
#[path = "../support/wechat_demo/layout.rs"]
mod layout;
#[path = "../support/wechat_demo/message_tracking.rs"]
mod message_tracking;
#[path = "../support/wechat_demo/pixel_motion.rs"]
mod pixel_motion;
#[path = "../support/wechat_demo/recovery.rs"]
mod recovery;
#[path = "../support/wechat_demo/scroll_motion.rs"]
mod scroll_motion;
#[path = "../support/wechat_demo/send_verification.rs"]
mod send_verification;
#[path = "../support/wechat_demo/snapshot.rs"]
mod snapshot;
#[path = "../support/wechat_demo/spatial.rs"]
mod spatial;
#[path = "../support/wechat_demo/text.rs"]
mod text;
use argusflow_capture_contracts::{PixelRect, ScreenRect};
use argusflow_core::ImagePoint;
use message_tracking::{Tracker, Uncertainty};
use send_verification::{Reason, SendOutcome, verify};
use std::{sync::Arc, time::Duration};

fn rect(x: u32, y: u32, w: u32, h: u32) -> PixelRect {
    PixelRect::new(x, y, w, h).unwrap()
}
fn bounds() -> ScreenRect {
    ScreenRect::new(0, 0, 800, 600).unwrap()
}
fn region() -> PixelRect {
    layout::Zone::Messages.region(bounds()).unwrap()
}
fn bubble(y: u32) -> PixelRect {
    rect(650, y, 70, 28)
}
fn tick(tracker: &mut Tracker, boxes: Vec<PixelRect>) {
    tracker.observe(boxes, Duration::from_millis(25), None);
}
fn block(value: &str, r: PixelRect) -> text::TextRecord {
    text::TextRecord {
        text: value.into(),
        confidence: 0.99,
        rect: r,
        polygon: [ImagePoint {
            x: r.x() as f32,
            y: r.y() as f32,
        }; 4],
    }
}
fn observation() -> snapshot::Observation {
    let mut frame = frame::Frame::from_bgrx(bounds(), vec![32; 800 * 600 * 4]).unwrap();
    paint(&mut frame, bubble(390), [100, 210, 60]);
    snapshot::Observation {
        frame,
        blocks: Arc::new(vec![
            block("搜索", rect(90, 40, 40, 20)),
            block("文件传输助手", rect(450, 40, 100, 20)),
            block("你好", rect(665, 394, 35, 19)),
        ]),
    }
}
fn paint(frame: &mut frame::Frame, r: PixelRect, bgr: [u8; 3]) {
    let pixels = Arc::make_mut(&mut frame.pixels);
    for y in r.y()..r.bottom() {
        for x in r.x()..r.right() {
            let offset = (y as usize * 800 + x as usize) * 3;
            pixels[offset..offset + 3].copy_from_slice(&bgr);
        }
    }
}

#[test]
fn saturated_repeated_bubbles_keep_count_but_new_track_is_identified() {
    let baseline: Vec<_> = (0..6).map(|n| bubble(85 + n * 60)).collect();
    let mut tracker = Tracker::new(region(), baseline.clone());
    let mut final_boxes = Vec::new();
    for step in 1..=6 {
        let mut boxes = Vec::new();
        for old in &baseline {
            let top = old.y().saturating_sub(step * 10).max(region().y());
            let bottom = old.bottom().saturating_sub(step * 10);
            if bottom > top + 12 {
                boxes.push(rect(old.x(), top, old.width(), bottom - top));
            }
        }
        if step >= 2 {
            boxes.push(bubble(445 - step * 10));
        }
        tick(&mut tracker, boxes.clone());
        final_boxes = boxes;
    }
    // 顶部一条离开、底部一条进入；可见数量仍为 6。
    assert_eq!(baseline.len(), final_boxes.len());
    tick(&mut tracker, final_boxes.clone());
    tick(&mut tracker, final_boxes);
    assert_eq!(tracker.candidate(), Ok(bubble(385)));
}

#[test]
fn identical_periodic_frames_cannot_prove_a_birth() {
    let boxes: Vec<_> = (0..6).map(|n| bubble(85 + n * 60)).collect();
    let mut tracker = Tracker::new(region(), boxes.clone());
    for _ in 0..5 {
        tick(&mut tracker, boxes.clone());
    }
    assert_eq!(tracker.candidate(), Err(Uncertainty::NoBirth));
}
#[test]
fn missed_frames_do_not_reassign_message_identity() {
    let mut tracker = Tracker::new(region(), vec![bubble(300)]);
    tracker.observe(
        vec![bubble(240), bubble(300)],
        Duration::from_millis(180),
        None,
    );
    assert_eq!(tracker.candidate(), Err(Uncertainty::CaptureGap));
}
#[test]
fn disappearing_middle_and_multiple_births_are_not_success() {
    let mut lost = Tracker::new(region(), vec![bubble(200), bubble(300)]);
    tick(&mut lost, vec![bubble(300)]);
    assert_eq!(lost.candidate(), Err(Uncertainty::AmbiguousMotion));
    let mut many = Tracker::new(region(), vec![]);
    for _ in 0..3 {
        tick(&mut many, vec![bubble(200), bubble(300)]);
    }
    assert_eq!(many.candidate(), Err(Uncertainty::MultipleBirths));
}
#[test]
fn incoming_bubble_does_not_become_outgoing_evidence() {
    let mut snapshot = observation();
    paint(&mut snapshot.frame, bubble(390), [32, 32, 32]);
    paint(&mut snapshot.frame, rect(330, 390, 70, 28), [100, 210, 60]);
    assert!(bubbles::outgoing(&snapshot.frame, region()).is_empty());
}
#[test]
fn ocr_and_empty_editor_without_birth_are_unconfirmed() {
    assert_eq!(
        verify(&observation(), Err(Uncertainty::NoBirth), "你好").unwrap(),
        SendOutcome::Unconfirmed(Reason::Tracking(Uncertainty::NoBirth))
    );
}
#[test]
fn exact_text_inside_new_outgoing_bubble_is_local_evidence_only() {
    let snapshot = observation();
    assert_eq!(
        bubbles::outgoing(&snapshot.frame, region()),
        vec![bubble(390)]
    );
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你好").unwrap(),
        SendOutcome::LocalBubbleObserved
    );
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "别的文字").unwrap(),
        SendOutcome::Unconfirmed(Reason::TextMismatch)
    );
}
#[test]
fn uncleared_draft_and_status_icons_block_confirmation() {
    let mut draft = observation();
    Arc::make_mut(&mut draft.blocks).push(block("你好", rect(400, 490, 35, 20)));
    assert_eq!(
        verify(&draft, Ok(bubble(390)), "你好").unwrap(),
        SendOutcome::Unconfirmed(Reason::DraftPresent)
    );
    for color in [[40, 40, 240], [130, 130, 130]] {
        let mut pending = observation();
        paint(&mut pending.frame, rect(627, 400, 10, 10), color);
        assert_eq!(
            verify(&pending, Ok(bubble(390)), "你好").unwrap(),
            SendOutcome::Unconfirmed(Reason::StatusNotClear)
        );
    }
}

#[test]
fn split_blocks_match_only_inside_the_same_bubble() {
    let mut snapshot = observation();
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你 好").unwrap(),
        SendOutcome::LocalBubbleObserved
    );
    let blocks = Arc::make_mut(&mut snapshot.blocks);
    blocks.pop();
    blocks.push(block("你", rect(655, 394, 15, 19)));
    blocks.push(block("好", rect(680, 394, 15, 19)));
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你 好").unwrap(),
        SendOutcome::LocalBubbleObserved
    );
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你 错").unwrap(),
        SendOutcome::Unconfirmed(Reason::TextMismatch)
    );
    Arc::make_mut(&mut snapshot.blocks).last_mut().unwrap().rect = rect(680, 350, 15, 19);
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你 好").unwrap(),
        SendOutcome::Unconfirmed(Reason::TextMismatch)
    );
    let last = Arc::make_mut(&mut snapshot.blocks).last_mut().unwrap();
    last.rect = rect(680, 394, 15, 19);
    last.confidence = 0.4;
    assert_eq!(
        verify(&snapshot, Ok(bubble(390)), "你 好").unwrap(),
        SendOutcome::Unconfirmed(Reason::TextMismatch)
    );
}
#[test]
fn new_bubble_must_remain_stable_on_later_frame() {
    let before = observation().frame;
    let mut after = before.clone();
    assert!(send_verification::stable(&before, &after, bubble(390)).unwrap());
    paint(&mut after, rect(660, 400, 10, 10), [10, 10, 10]);
    assert!(!send_verification::stable(&before, &after, bubble(390)).unwrap());
}

#[test]
fn green_caret_is_excluded_by_pixels_but_real_letter_i_is_not() {
    let mut snapshot = observation();
    let r = rect(400, 490, 9, 27);
    Arc::make_mut(&mut snapshot.blocks).push(block("I", r));
    paint(&mut snapshot.frame, rect(403, 493, 2, 20), [100, 210, 60]);
    assert!(editor::is_empty(
        &snapshot,
        layout::Zone::Editor.region(bounds()).unwrap()
    ));
    paint(&mut snapshot.frame, rect(403, 493, 2, 20), [240, 240, 240]);
    assert!(!editor::is_empty(
        &snapshot,
        layout::Zone::Editor.region(bounds()).unwrap()
    ));
}

#[test]
fn unique_global_translation_recovers_large_instant_scroll() {
    let old = vec![rect(600, 120, 120, 35), rect(640, 260, 80, 28), bubble(390)];
    let new = vec![
        rect(600, 80, 120, 15),
        rect(640, 200, 80, 28),
        bubble(330),
        bubble(390),
    ];
    // 顶部发生裁剪，其他两种形状提供同一 -60px 位移的唯一支持。
    let mut tracker = Tracker::new(rect(280, 80, 510, 380), old);
    for _ in 0..3 {
        tick(&mut tracker, new.clone());
    }
    assert_eq!(tracker.candidate(), Ok(bubble(390)));
}

#[test]
fn periodic_geometry_is_not_used_for_large_scroll_guessing() {
    let boxes = vec![bubble(120), bubble(180), bubble(240), bubble(300)];
    assert_eq!(scroll_motion::estimate(&boxes, &boxes, region()), None);
}

#[test]
fn pixel_registration_uses_unique_texture_when_all_message_bubbles_repeat() {
    let viewport = rect(280, 80, 500, 420);
    let boxes: Vec<_> = (0..7).map(|i| bubble(90 + i * 60)).collect();
    let mut before = frame::Frame::from_bgrx(bounds(), vec![32; 800 * 600 * 4]).unwrap();
    for b in &boxes {
        paint(&mut before, *b, [100, 210, 60]);
    }
    let mut after = before.clone();
    // 相同形状、相同数量的消息，仅时间标签的位置能打破周期歧义。
    paint(&mut before, rect(500, 285, 35, 8), [150, 150, 150]);
    paint(&mut after, rect(500, 225, 35, 8), [150, 150, 150]);
    assert_eq!(
        pixel_motion::estimate(&before, &after, viewport, &boxes, &boxes),
        Some(-60)
    );
    let mut tracker = Tracker::new(viewport, boxes.clone());
    tracker.observe(boxes.clone(), Duration::from_millis(25), Some(-60));
    tick(&mut tracker, boxes.clone());
    tick(&mut tracker, boxes);
    assert_eq!(tracker.candidate(), Ok(bubble(450)));
}

#[test]
fn perfectly_periodic_pixels_reject_multiple_equal_registration_peaks() {
    let viewport = rect(280, 80, 500, 420);
    let boxes: Vec<_> = (0..7).map(|i| bubble(90 + i * 60)).collect();
    let mut before = frame::Frame::from_bgrx(bounds(), vec![32; 800 * 600 * 4]).unwrap();
    for b in &boxes {
        paint(&mut before, *b, [100, 210, 60]);
    }
    assert_eq!(
        pixel_motion::estimate(&before, &before, viewport, &boxes, &boxes),
        None
    );
}
