use argusflow_agent::VisualVerificationResult;
use argusflow_core::{TargetWaitMode, TargetWaitPolicy};

use super::{
    match_added_deadline_result, match_present_deadline_result, match_removed_deadline_result,
};

/// 创建验证器纯结果函数使用的固定有界等待策略。
fn bounded_wait() -> TargetWaitPolicy {
    TargetWaitPolicy {
        mode: TargetWaitMode::Bounded,
        timeout_ms: 5_000,
        poll_interval_ms: 150,
    }
}

#[test]
fn deadline_preserves_last_match_added_evidence() {
    let result = match_added_deadline_result(
        true,
        "动作后上下文 #1 不再严格唯一（命中 0 项）",
        bounded_wait(),
    );

    assert_eq!(
        result,
        VisualVerificationResult::Rejected {
            reason: "动作后 5000ms 内未确认新增匹配：动作后上下文 #1 不再严格唯一（命中 0 项）"
                .to_owned(),
        },
    );
}

#[test]
fn deadline_without_a_fresh_scene_remains_uncertain() {
    let result = match_present_deadline_result(None, bounded_wait());

    assert_eq!(
        result,
        VisualVerificationResult::Uncertain {
            reason: "在 5000ms 内未取得晚于动作前基线的完整视觉 Scene".to_owned(),
        },
    );
}

#[test]
fn removed_deadline_preserves_last_spatial_evidence() {
    let result = match_removed_deadline_result(
        true,
        "动作前 1 个匹配在当前 1 个匹配中仍有同文本相交实例，没有发现旧实例消失",
        bounded_wait(),
    );

    assert_eq!(
        result,
        VisualVerificationResult::Rejected {
            reason: "动作后 5000ms 内未确认匹配消失：动作前 1 个匹配在当前 1 个匹配中仍有同文本相交实例，没有发现旧实例消失"
                .to_owned(),
        },
    );
}
