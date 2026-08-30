//! VisualScene cache 的 freshness、generation 和 dirty ROI 失效规则。

use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;

use crate::{
    diff::DirtyMap,
    frame::{PhysicalRect, TopologyGeneration},
};

use super::{
    FreshRegion, ObservationCoverage, ObservationState, VisualScene, observation::duration_millis,
};

/// cache lookup 的失败原因，供 Planner Explain/Inspector 使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMissReason {
    /// 尚未构建 scene。
    Empty,
    /// scene 属于另一个窗口身份。
    WindowMismatch,
    /// scene 属于较旧的窗口拓扑。
    TopologyMismatch,
    /// scene 超过调用方 freshness。
    Expired,
    /// 查询 ROI 与 dirty map 相交。
    Dirty,
}

/// VisualScene cache 查询结果。
#[derive(Debug, Clone)]
pub enum CacheLookup {
    /// 命中一份仍然可复用的场景。
    Hit(Arc<VisualScene>),
    /// 未命中及其明确原因。
    Miss(CacheMissReason),
}

#[derive(Debug)]
struct CacheState {
    /// 最近一份稳定场景。
    scene: Option<Arc<VisualScene>>,
    /// 仍未被新 OCR 覆盖的 dirty ROI。
    dirty_regions: Vec<PhysicalRect>,
    /// 每个已经重新识别的区域的独立 freshness 起点。
    fresh_regions: Vec<(PhysicalRect, Instant)>,
    /// 最近 scene 是否源自完整 viewport bootstrap。
    coverage: ObservationCoverage,
}

impl Default for CacheState {
    fn default() -> Self {
        Self {
            scene: None,
            dirty_regions: Vec::new(),
            fresh_regions: Vec::new(),
            coverage: ObservationCoverage::Empty,
        }
    }
}

/// 只缓存最近稳定 VisualScene，不缓存 PNG 或跨页历史。
#[derive(Debug, Default)]
pub struct VisualSceneCache {
    /// 短时读写锁；不跨 await 持有。
    state: RwLock<CacheState>,
}

impl VisualSceneCache {
    /// 创建空 cache。
    pub fn new() -> Self {
        Self::default()
    }

    /// 替换 current scene，同时清除已由新 scene 覆盖的 dirty 标记。
    pub fn replace(&self, scene: Arc<VisualScene>) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.scene = Some(scene);
        state.dirty_regions.clear();
        let now = Instant::now();
        state.fresh_regions = state
            .scene
            .as_ref()
            .map(|scene| vec![(scene.viewport, now)])
            .unwrap_or_default();
        state.coverage = ObservationCoverage::Complete;
    }

    /// 写入一次局部 OCR 结果，只清除被该刷新区域完整覆盖的 dirty ROI。
    ///
    /// 其它 dirty ROI 仍然保留，避免一次局部查询把未重新识别的旧节点误报为新鲜事实。
    pub fn replace_region(&self, scene: Arc<VisualScene>, refreshed_region: PhysicalRect) {
        self.replace_regions(scene, &[refreshed_region]);
    }

    /// 写入多块局部 OCR 结果，并分别维护各区域的 freshness。
    pub fn replace_regions(&self, scene: Arc<VisualScene>, refreshed_regions: &[PhysicalRect]) {
        if refreshed_regions.is_empty() {
            return;
        }
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let had_previous_scene = state.scene.is_some();
        let scene_viewport = scene.viewport;
        state.scene = Some(scene);
        let now = Instant::now();
        if !had_previous_scene {
            // 首次局部 OCR 结果不能伪装成完整 scene；保留 viewport 未观测部分的
            // dirty 标记，避免调用方把缺失节点当成已经识别的事实。
            state.dirty_regions = subtract_regions(scene_viewport, refreshed_regions);
            state.fresh_regions = refreshed_regions
                .iter()
                .copied()
                .map(|region| (region, now))
                .collect();
            state.coverage = ObservationCoverage::Partial {
                covered: refreshed_regions.to_vec(),
            };
            return;
        }
        let mut remaining_dirty = Vec::new();
        for dirty in state.dirty_regions.drain(..) {
            let mut fragments = vec![dirty];
            for refreshed in refreshed_regions {
                fragments = fragments
                    .into_iter()
                    .flat_map(|fragment| subtract_region(fragment, *refreshed))
                    .collect();
            }
            remaining_dirty.extend(fragments);
        }
        state.dirty_regions = remaining_dirty;
        let mut remaining_fresh = Vec::new();
        for (fresh, stored_at) in state.fresh_regions.drain(..) {
            let mut fragments = vec![fresh];
            for refreshed in refreshed_regions {
                fragments = fragments
                    .into_iter()
                    .flat_map(|fragment| subtract_region(fragment, *refreshed))
                    .collect();
            }
            remaining_fresh.extend(fragments.into_iter().map(|fragment| (fragment, stored_at)));
        }
        remaining_fresh.extend(
            refreshed_regions
                .iter()
                .copied()
                .map(|region| (region, now)),
        );
        state.fresh_regions = remaining_fresh;
    }

    /// 用差分结果只使相交区域失效。
    pub fn invalidate(&self, dirty: &DirtyMap) {
        if dirty.regions.is_empty() {
            return;
        }
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for region in &dirty.regions {
            if !state.dirty_regions.contains(&region.rect) {
                state.dirty_regions.push(region.rect);
            }
        }
    }

    /// 按窗口、topology、freshness 和 query ROI 查询 cache。
    pub fn lookup(
        &self,
        window: WindowIdentity,
        topology_generation: TopologyGeneration,
        max_age: Duration,
        query_region: Option<PhysicalRect>,
    ) -> CacheLookup {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(scene) = &state.scene else {
            return CacheLookup::Miss(CacheMissReason::Empty);
        };
        if scene.window != window {
            return CacheLookup::Miss(CacheMissReason::WindowMismatch);
        }
        if !topology_generation.is_unknown() && scene.topology_generation != topology_generation {
            return CacheLookup::Miss(CacheMissReason::TopologyMismatch);
        }
        if query_region.is_none() && !state.coverage.is_complete() {
            // 整窗查询必须建立在完整 bootstrap 上；即使若干局部 freshness
            // 恰好拼满 viewport，也不能把 Partial 状态提升为完整场景事实。
            return CacheLookup::Miss(CacheMissReason::Expired);
        }
        let fresh = query_region
            .map(|region| region_is_fresh(region, &state.fresh_regions, max_age))
            .unwrap_or_else(|| region_is_fresh(scene.viewport, &state.fresh_regions, max_age));
        if !fresh {
            return CacheLookup::Miss(CacheMissReason::Expired);
        }
        let dirty = match query_region {
            Some(region) => state
                .dirty_regions
                .iter()
                .any(|dirty| dirty.intersects(region)),
            None => !state.dirty_regions.is_empty(),
        };
        if dirty {
            return CacheLookup::Miss(CacheMissReason::Dirty);
        }
        CacheLookup::Hit(scene.clone())
    }

    /// 返回当前场景，仅供 runtime/inspector 使用，不绕过 freshness 检查。
    pub fn current(&self) -> Option<Arc<VisualScene>> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .scene
            .clone()
    }

    /// 返回不暴露内部锁和 `Instant` 的只读观测状态。
    pub fn observation(&self) -> ObservationState {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ObservationState {
            coverage: state.coverage.clone(),
            fresh_regions: state
                .fresh_regions
                .iter()
                .map(|(region, stored_at)| FreshRegion {
                    region: *region,
                    age_ms: duration_millis(stored_at.elapsed()),
                })
                .collect(),
            dirty_regions: state.dirty_regions.clone(),
        }
    }

    /// 判断当前场景是否仍有尚未重新识别的 dirty 区域。
    pub fn has_dirty_regions(&self) -> bool {
        !self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .dirty_regions
            .is_empty()
    }

    /// 返回尚未被 OCR 成功覆盖的 Dirty ROI 快照，供下一次刷新继续处理。
    pub fn pending_dirty_regions(&self) -> Vec<PhysicalRect> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .dirty_regions
            .clone()
    }

    /// 判断目标 ROI 是否与尚未重新识别的区域相交。
    pub fn is_region_dirty(&self, region: PhysicalRect) -> bool {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .dirty_regions
            .iter()
            .any(|dirty| dirty.intersects(region))
    }

    /// 清除窗口关闭或拓扑重建后的所有短期事实。
    pub fn clear(&self) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *state = CacheState::default();
    }
}

/// 判断一个查询区域是否被尚未过期的 freshness 矩形完整覆盖。
fn region_is_fresh(
    query: PhysicalRect,
    fresh_regions: &[(PhysicalRect, Instant)],
    max_age: Duration,
) -> bool {
    let mut uncovered = vec![query];
    for (fresh, stored_at) in fresh_regions {
        if stored_at.elapsed() > max_age {
            continue;
        }
        uncovered = uncovered
            .into_iter()
            .flat_map(|region| subtract_region(region, *fresh))
            .collect();
        if uncovered.is_empty() {
            return true;
        }
    }
    false
}

/// 从一个基础区域中连续移除多块刷新区域，返回仍未覆盖的边带集合。
fn subtract_regions(base: PhysicalRect, refreshed_regions: &[PhysicalRect]) -> Vec<PhysicalRect> {
    refreshed_regions
        .iter()
        .copied()
        .fold(vec![base], |remaining, refreshed| {
            remaining
                .into_iter()
                .flat_map(|region| subtract_region(region, refreshed))
                .collect()
        })
}

/// 从一个 dirty rectangle 中移除已重新识别的区域，保留未覆盖的四个边带。
fn subtract_region(dirty: PhysicalRect, refreshed: PhysicalRect) -> Vec<PhysicalRect> {
    if !dirty.intersects(refreshed) {
        return vec![dirty];
    }
    let left = i64::from(dirty.x).max(i64::from(refreshed.x));
    let top = i64::from(dirty.y).max(i64::from(refreshed.y));
    let right = dirty.right().min(refreshed.right());
    let bottom = dirty.bottom().min(refreshed.bottom());
    let mut remaining = Vec::with_capacity(4);
    push_edges(
        &mut remaining,
        dirty.x as i64,
        dirty.y as i64,
        dirty.right(),
        top,
    );
    push_edges(
        &mut remaining,
        dirty.x as i64,
        bottom,
        dirty.right(),
        dirty.bottom(),
    );
    push_edges(&mut remaining, dirty.x as i64, top, left, bottom);
    push_edges(&mut remaining, right, top, dirty.right(), bottom);
    remaining
}

/// 把有符号边界转换为有效的物理矩形；空边带直接丢弃。
fn push_edges(output: &mut Vec<PhysicalRect>, left: i64, top: i64, right: i64, bottom: i64) {
    if right <= left || bottom <= top {
        return;
    }
    let Ok(x) = i32::try_from(left) else {
        return;
    };
    let Ok(y) = i32::try_from(top) else {
        return;
    };
    let Ok(width) = u32::try_from(right - left) else {
        return;
    };
    let Ok(height) = u32::try_from(bottom - top) else {
        return;
    };
    if let Some(rect) = PhysicalRect::new(x, y, width, height) {
        output.push(rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frame::{FrameId, QpcTimestamp},
        image::CapturedFrame,
        ocr::{OcrModel, OcrPreprocessingSummary, OcrRequestId, OcrResponse},
        scene::{SceneBuildOptions, VisualSceneBuilder},
    };

    #[test]
    fn partial_bootstrap_never_satisfies_a_complete_scene_lookup() {
        let scene = fixture_scene();
        let cache = VisualSceneCache::new();
        let partial = PhysicalRect::new(0, 0, 50, 100).expect("partial region is valid");

        cache.replace_regions(scene.clone(), &[partial]);

        assert!(matches!(
            cache.observation().coverage,
            ObservationCoverage::Partial { .. }
        ));
        assert!(matches!(
            cache.lookup(
                scene.window,
                scene.topology_generation,
                Duration::from_secs(1),
                None,
            ),
            CacheLookup::Miss(CacheMissReason::Expired)
        ));

        cache.replace(scene.clone());

        assert!(cache.observation().coverage.is_complete());
        assert!(matches!(
            cache.lookup(
                scene.window,
                scene.topology_generation,
                Duration::from_secs(1),
                None,
            ),
            CacheLookup::Hit(_)
        ));
    }

    /// 构造无需真实捕获源和 OCR worker 的空文本场景。
    fn fixture_scene() -> Arc<VisualScene> {
        let window = WindowIdentity {
            handle: 1,
            process_id: 2,
        };
        let frame = CapturedFrame::from_bgra8(
            FrameId::new(1),
            TopologyGeneration::new(1),
            window,
            QpcTimestamp::new(1),
            100,
            100,
            96,
            96,
            400,
            vec![0; 100 * 100 * 4],
        )
        .expect("fixture frame is valid");
        let response = OcrResponse {
            request_id: OcrRequestId::new(),
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            model: OcrModel::PpOcrV6Small,
            elapsed_ms: 1,
            preprocessing: OcrPreprocessingSummary {
                input_width: 100,
                input_height: 100,
                output_width: 100,
                output_height: 100,
                contrast_enhanced: false,
                sharpened: false,
                binarized: false,
            },
            timings: crate::ocr::OcrTimingSummary {
                preprocess_elapsed_ms: 0,
                inference_elapsed_ms: 1,
            },
            model_input: None,
            items: Vec::new(),
        };
        VisualSceneBuilder::new()
            .build(window, &frame, &[response], &SceneBuildOptions::default())
            .expect("fixture scene builds")
    }
}
