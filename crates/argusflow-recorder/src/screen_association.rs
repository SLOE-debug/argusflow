//! 输入与已持久化画面的时间关联；不推断文字或因果关系。

use crate::{EventEvidence, InputPhase, RawInput, RecordingTrace};
use std::collections::HashMap;

/// 按输入锚点分配变化区间，松键不截断按下后的视觉反馈。
pub(crate) fn associate(trace: &mut RecordingTrace) {
    // 后处理可能删除重复帧；所有事件先撤销旧引用，包括不作为锚点的 KeyUp。
    for event in &mut trace.timeline.events {
        if let Some(evidence) = &mut event.evidence {
            evidence.screen = Default::default();
        }
    }
    let anchors = trace
        .timeline
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| match event.input {
            RawInput::Key {
                phase: InputPhase::Down,
                ..
            }
            | RawInput::Mouse { .. }
            | RawInput::Wheel { .. }
            | RawInput::PointerMotion(_)
            | RawInput::Move { .. } => true,
            _ => false,
        })
        .map(|(index, event)| (index, event.elapsed_ms.saturating_mul(1000)))
        .collect::<Vec<_>>();
    for (position, (index, timestamp)) in anchors.iter().enumerate() {
        let end = anchors
            .get(position + 1)
            .map_or(u64::MAX, |(_, timestamp)| *timestamp);
        let evidence = trace.timeline.events[*index]
            .evidence
            .get_or_insert_with(EventEvidence::default);
        evidence.screen.after.clear();
        let mut before = HashMap::new();
        for frame in &trace.screen.frames {
            if frame.presented_us <= *timestamp {
                let current = before.entry(frame.source).or_insert(frame);
                if frame.presented_us >= current.presented_us {
                    *current = frame;
                }
            } else if frame.presented_us < end {
                evidence.screen.after.push(frame.id);
            }
        }
        evidence.screen.before = before.values().map(|frame| frame.id).collect();
        evidence.screen.before.sort();
    }
}
