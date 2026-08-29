//! 滚动闭环：位移观测、连续性验收和 current page 替换。

use std::collections::BTreeSet;

use argusflow_core::WindowIdentity;
use serde::{Deserialize, Serialize};

use super::{
    history::ScrollDocumentHistory,
    model::{
        AnchorMatchEvidence, PageSnapshot, ScrollCalibration, ScrollDirection, ScrollRegion,
        WheelSteps, match_anchors,
    },
};
use crate::error::VisionError;

/// 前后稳定页面的连续性验收结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PageTransition {
    /// 已证明 overlap 足够，页面可追加到 history。
    Accepted {
        /// 以旧页锚点数量归一化的 overlap 证据。
        overlap_ratio: f32,
        /// 文本匹配数。
        text_matches: usize,
        /// patch 匹配数。
        patch_matches: usize,
    },
    /// 实际位移太小，需要小批次继续滚动。
    Undershot {
        /// 实际位移。
        actual_shift_px: f32,
    },
    /// 位移过大且无法建立连续性，应反向小步恢复。
    Overshot {
        /// 实际位移。
        actual_shift_px: f32,
    },
    /// 内容发生变化但无法证明是同一列表连续页。
    ContinuityUnproven {
        /// 已找到的锚点匹配证据。
        evidence: AnchorMatchEvidence,
    },
    /// wheel 后没有检测到有效内容位移。
    NoMovement,
}

impl PageTransition {
    /// 判断是否已经可以把新页加入 history。
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

/// 一次已经被接受的页面推进结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedPage {
    /// 新页面中新增的 history 项。
    pub added_items: usize,
    /// overlap 中被去重的项。
    pub deduplicated_items: usize,
    /// 新页面序号。
    pub page_index: u32,
}

/// 单窗口、单滚动区域的视觉分页会话。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollSession {
    /// 被滚动的窗口身份。
    pub window: WindowIdentity,
    /// 滚动内容区域。
    pub region: ScrollRegion,
    /// 内容移动方向。
    pub direction: ScrollDirection,
    /// 从零开始的当前页序号。
    pub page_index: u32,
    /// 当前 viewport 对应的最新 page snapshot。
    pub current_page: Option<PageSnapshot>,
    /// 已接受页面拼接出来的 history。
    pub history: ScrollDocumentHistory,
    /// 当前列表的位移校准。
    pub calibration: ScrollCalibration,
    /// 已连续恢复/反向尝试的次数。
    pub recovery_attempts: u8,
}

impl ScrollSession {
    /// 创建尚未观察第一页的滚动会话。
    pub fn new(
        window: WindowIdentity,
        region: ScrollRegion,
        direction: ScrollDirection,
    ) -> Result<Self, VisionError> {
        let calibration = ScrollCalibration::default();
        calibration.validate()?;
        Ok(Self {
            window,
            region,
            direction,
            page_index: 0,
            current_page: None,
            history: ScrollDocumentHistory::new(),
            calibration,
            recovery_attempts: 0,
        })
    }

    /// 安装第一份稳定页，并把其全部内容写入 history。
    pub fn start(&mut self, page: PageSnapshot) -> Result<(), VisionError> {
        self.validate_page(&page)?;
        let overlap = BTreeSet::new();
        self.history.append_page(&page, &overlap);
        self.current_page = Some(page);
        self.page_index = 0;
        self.recovery_attempts = 0;
        Ok(())
    }

    /// 依据剩余目标位移生成下一批 wheel 输入。
    pub fn next_batch(&self, accumulated_shift_px: f32) -> Option<WheelSteps> {
        let current = self.current_page.as_ref()?;
        let target = self.calibration.target_shift(current.region.bounds.height);
        self.calibration
            .estimate_batch(self.direction, (target - accumulated_shift_px).max(0.0))
    }

    /// 观测一次 wheel 产生的实际位移并更新 EMA。
    pub fn observe_displacement(&mut self, steps: WheelSteps, actual_shift_px: f32) {
        self.calibration.update(steps, actual_shift_px);
    }

    /// 对新稳定页做 continuity/page acceptance 判定。
    pub fn evaluate_page(
        &self,
        page: &PageSnapshot,
        actual_shift_px: f32,
    ) -> Result<PageTransition, VisionError> {
        self.validate_page(page)?;
        let Some(previous) = &self.current_page else {
            return Err(VisionError::Protocol {
                message: "cannot evaluate a page before ScrollSession::start".to_owned(),
            });
        };
        if !actual_shift_px.is_finite() || actual_shift_px < 0.0 {
            return Err(VisionError::Protocol {
                message: "actual scroll displacement must be finite and non-negative".to_owned(),
            });
        }
        if actual_shift_px < self.calibration.min_shift_px {
            return Ok(PageTransition::NoMovement);
        }
        let target = self.calibration.target_shift(previous.region.bounds.height);
        let evidence = match_anchors(previous, page);
        let overlap_ratio = evidence.matched as f32 / previous.anchors.len().max(1) as f32;
        if actual_shift_px > target * 1.35 && evidence.matched == 0 {
            return Ok(PageTransition::Overshot { actual_shift_px });
        }
        if actual_shift_px < target * 0.5 {
            return Ok(PageTransition::Undershot { actual_shift_px });
        }
        if (0.15..=0.45).contains(&overlap_ratio)
            && evidence.text_matches > 0
            && evidence.patch_matches > 0
        {
            Ok(PageTransition::Accepted {
                overlap_ratio,
                text_matches: evidence.text_matches,
                patch_matches: evidence.patch_matches,
            })
        } else {
            Ok(PageTransition::ContinuityUnproven { evidence })
        }
    }

    /// 只有验收成功后才替换 current page 并追加 history。
    pub fn accept_page(
        &mut self,
        page: PageSnapshot,
        transition: &PageTransition,
    ) -> Result<AcceptedPage, VisionError> {
        self.validate_page(&page)?;
        if !transition.is_accepted() {
            return Err(VisionError::VerificationRejected {
                reason: "page transition has not proved overlap continuity".to_owned(),
            });
        }
        let previous = self
            .current_page
            .as_ref()
            .ok_or_else(|| VisionError::Protocol {
                message: "cannot accept a page before ScrollSession::start".to_owned(),
            })?;
        let overlap_signatures = previous
            .anchors
            .iter()
            .filter_map(|old_anchor| {
                page.items.iter().find_map(|item| {
                    let same_text = old_anchor
                        .text
                        .as_ref()
                        .is_some_and(|text| text == &item.text);
                    (same_text || old_anchor.patch_hash == item.patch_hash)
                        .then_some(item.signature)
                })
            })
            .collect::<BTreeSet<_>>();
        let append = self.history.append_page(&page, &overlap_signatures);
        self.current_page = Some(page);
        self.page_index = self.page_index.saturating_add(1);
        self.recovery_attempts = 0;
        Ok(AcceptedPage {
            added_items: append.added,
            deduplicated_items: append.deduplicated,
            page_index: self.page_index,
        })
    }

    /// 记录一次 overshoot recovery，超过上限后返回稳定错误。
    pub fn record_recovery(&mut self, max_attempts: u8) -> Result<(), VisionError> {
        if self.recovery_attempts >= max_attempts {
            return Err(VisionError::ScrollOvershot);
        }
        self.recovery_attempts = self.recovery_attempts.saturating_add(1);
        Ok(())
    }

    /// 校验新页属于同一窗口、同一滚动区域，并且没有越界。
    pub(crate) fn validate_page(&self, page: &PageSnapshot) -> Result<(), VisionError> {
        if page.window != self.window {
            return Err(VisionError::WindowIdentityChanged {
                expected: self.window,
                actual: Some(page.window),
            });
        }
        if page.region != self.region {
            return Err(VisionError::Protocol {
                message: "page belongs to a different scroll region".to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frame::{FrameId, PhysicalRect},
        scene::VisualNodeId,
        scroll::model::{PageItem, ScrollAnchor},
    };

    fn page(
        window: WindowIdentity,
        scene_id: u64,
        items: Vec<PageItem>,
        anchors: Vec<ScrollAnchor>,
    ) -> PageSnapshot {
        PageSnapshot {
            window,
            scene_id: crate::scene::SceneId::new(scene_id),
            frame_id: FrameId::new(scene_id),
            region: ScrollRegion::new(PhysicalRect::new(0, 0, 100, 100).expect("fixture rect")),
            content_signature: scene_id,
            items,
            anchors,
        }
    }

    fn item(id: u64, text: &str, patch_hash: u64) -> PageItem {
        PageItem {
            node_id: VisualNodeId::new(id),
            text: text.to_owned(),
            bbox: PhysicalRect::new(id as i32, 70, 10, 10).expect("fixture rect"),
            confidence: 0.9,
            patch_hash,
            signature: patch_hash,
        }
    }

    fn anchor(id: u64, text: &str, patch_hash: u64) -> ScrollAnchor {
        ScrollAnchor {
            node_id: VisualNodeId::new(id),
            text: Some(text.to_owned()),
            bbox: PhysicalRect::new(id as i32, 70, 10, 10).expect("fixture rect"),
            patch_hash,
            uniqueness: 1.0,
        }
    }

    #[test]
    fn accepts_only_a_page_with_text_and_patch_overlap() {
        let window = WindowIdentity {
            handle: 1,
            process_id: 2,
        };
        let old = page(
            window,
            1,
            vec![
                item(1, "a", 11),
                item(2, "b", 12),
                item(3, "c", 13),
                item(4, "d", 14),
            ],
            vec![
                anchor(1, "a", 11),
                anchor(2, "b", 12),
                anchor(3, "c", 13),
                anchor(4, "d", 14),
            ],
        );
        let new = page(window, 2, vec![item(1, "a", 11), item(5, "e", 15)], vec![]);
        let mut session =
            ScrollSession::new(window, old.region, ScrollDirection::Down).expect("fixture session");
        session.start(old).expect("fixture page");
        let transition = session.evaluate_page(&new, 82.0).expect("transition");
        assert!(transition.is_accepted());
        let accepted = session
            .accept_page(new, &transition)
            .expect("accepted page");
        assert_eq!(accepted.added_items, 1);
        assert_eq!(accepted.deduplicated_items, 1);
        assert_eq!(session.history.page_count(), 2);
        assert_eq!(
            session
                .current_page
                .as_ref()
                .expect("current page")
                .scene_id
                .get(),
            2
        );
    }

    #[test]
    fn rejects_unproven_large_movement_as_overshoot() {
        let window = WindowIdentity {
            handle: 1,
            process_id: 2,
        };
        let old = page(window, 1, vec![item(1, "a", 11)], vec![anchor(1, "a", 11)]);
        let new = page(window, 2, vec![item(2, "z", 99)], vec![]);
        let mut session =
            ScrollSession::new(window, old.region, ScrollDirection::Down).expect("fixture session");
        session.start(old).expect("fixture page");
        assert!(matches!(
            session.evaluate_page(&new, 200.0).expect("transition"),
            PageTransition::Overshot { .. }
        ));
    }
}
