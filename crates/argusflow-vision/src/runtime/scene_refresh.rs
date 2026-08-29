//! 稳定帧获取、OCR 刷新规划和视觉场景合并。

use std::{sync::Arc, time::Instant};

use argusflow_core::WindowIdentity;

use super::{SceneRefreshPolicy, VisionRuntime};
use crate::{
    error::{SceneExecutionPhase, VisionError},
    ocr::{OcrEngine, OcrRequest},
    refresh::{RefreshPlan, choose_refresh_plan},
    region::normalized_region_to_physical,
    scene::{CacheLookup, SceneBuildOptions, VisualScene},
    scene_execution::SceneExecutionTrace,
    stability::StableFrameGate,
};

impl VisionRuntime {
    /// 获取一份 cache-first 或 force-refresh 的稳定场景。
    pub async fn current_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualScene>, VisionError> {
        self.current_scene_inner(window, policy, None).await
    }

    /// 获取稳定场景，并把端到端执行阶段提供给独立的截止时间任务。
    pub(crate) async fn current_scene_traced(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
        trace: &SceneExecutionTrace,
    ) -> Result<Arc<VisualScene>, VisionError> {
        self.current_scene_inner(window, policy, Some(trace)).await
    }

    /// 执行场景刷新；轨迹为空时保持普通服务调用不产生诊断状态。
    async fn current_scene_inner(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
        trace: Option<&SceneExecutionTrace>,
    ) -> Result<Arc<VisualScene>, VisionError> {
        enter_phase(trace, SceneExecutionPhase::CacheLookup);
        let cache_lookup = self.lookup_cache(window, policy);
        if !policy.force_refresh {
            if let CacheLookup::Hit(scene) = &cache_lookup {
                let query_region = policy
                    .query_region
                    .or_else(|| {
                        policy.normalized_query_region.and_then(|region| {
                            normalized_region_to_physical(region, scene.viewport)
                        })
                    })
                    .unwrap_or(scene.viewport);
                self.metrics.record_query_pixels(query_region.area());
                self.metrics.record_refresh_plan(&RefreshPlan::CacheOnly {
                    reason: crate::refresh::RefreshReason::CacheValid,
                });
                self.metrics.record_scene_query();
                enter_phase(trace, SceneExecutionPhase::Completed);
                return Ok(scene.clone());
            }
        }

        enter_phase(trace, SceneExecutionPhase::OpeningCapture);
        let scope = self.scopes.get_or_create(window, policy.capture);
        let subscription = match self.subscription(&scope, window, policy.capture).await {
            Ok(subscription) => subscription,
            Err(error) => {
                self.scopes.remove(window, policy.capture);
                return Err(error);
            }
        };
        if subscription.window() != window {
            self.scopes.remove(window, policy.capture);
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(subscription.window()),
            });
        }

        enter_phase(trace, SceneExecutionPhase::WaitingForStableFrame);
        let stable_started_at = Instant::now();
        let mut gate = StableFrameGate::new(policy.stability, policy.diff)?;
        let frame = gate.wait_for_stable(subscription.as_ref()).await?;
        self.metrics
            .record_stable_frame_latency(stable_started_at.elapsed());
        if frame.window != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(frame.window),
            });
        }
        if !policy.topology_generation.is_unknown()
            && policy.topology_generation != frame.topology_generation
        {
            return Err(VisionError::OcrCancelled {
                reason: "captured frame topology is newer than the prepared policy".to_owned(),
            });
        }

        enter_phase(trace, SceneExecutionPhase::PlanningRefresh);
        let health = self.health();
        self.metrics
            .record_capture(frame.width as u64 * frame.height as u64);
        self.metrics
            .record_worker_queue_depth(health.worker.queue_depth);
        let dirty = self
            .update_cache_invalidation(&scope, window, &frame, policy.diff)
            .await?;
        if let Some(dirty) = &dirty {
            self.metrics
                .record_dirty_pixels(dirty.regions.iter().map(|region| region.rect.area()).sum());
        }
        let query_region = policy.query_region.or_else(|| {
            policy
                .normalized_query_region
                .and_then(|region| normalized_region_to_physical(region, frame.bounds()))
        });
        let cache = self
            .scopes
            .cache_for(window, policy.capture)
            .ok_or_else(|| VisionError::CaptureUnavailable {
                message: "visual scope cache disappeared during scene refresh".to_owned(),
            })?;
        self.metrics
            .record_query_pixels(query_region.unwrap_or(frame.bounds()).area());
        let base_scene = cache.current();
        let refresh_plan = choose_refresh_plan(
            dirty.as_ref(),
            frame.bounds(),
            base_scene.is_some(),
            matches!(
                cache_lookup,
                CacheLookup::Miss(
                    crate::scene::CacheMissReason::Expired | crate::scene::CacheMissReason::Dirty
                )
            ),
            policy.force_full_ocr,
            policy.diff.full_refresh_dirty_ratio,
        );
        self.metrics.record_refresh_plan(&refresh_plan);
        if let RefreshPlan::CacheOnly { .. } = &refresh_plan {
            let Some(scene) = base_scene else {
                return Err(VisionError::OcrFailed {
                    message: "refresh planner selected cache-only without a base scene".to_owned(),
                });
            };
            self.metrics.record_scene_query();
            enter_phase(trace, SceneExecutionPhase::Completed);
            return Ok(scene);
        }
        if !health.worker_ready {
            return Err(VisionError::WorkerUnavailable {
                message: health.worker.worker_version,
            });
        }

        let full_refresh = refresh_plan.is_full();
        let refresh_regions = if full_refresh {
            vec![frame.bounds()]
        } else {
            refresh_plan.regions().to_vec()
        };
        let mut responses = Vec::with_capacity(refresh_regions.len());
        for roi in &refresh_regions {
            enter_phase(trace, SceneExecutionPhase::PreparingOcrInput);
            let request = OcrRequest::from_frame(
                window,
                frame.frame_id,
                frame.topology_generation,
                &frame,
                *roi,
                policy.ocr.clone(),
                policy.ocr_timeout,
            )?;
            if let Some(trace) = trace {
                trace.record_ocr_input(&request);
            }
            let request_frame_id = request.frame_id;
            let request_generation = request.topology_generation;
            enter_phase(trace, SceneExecutionPhase::WaitingForWorker);
            let ocr_started_at = Instant::now();
            let response = self.worker.recognize(request.clone()).await?;
            self.metrics
                .record_ocr_latency(request.profile.model, ocr_started_at.elapsed());
            self.metrics.record_ocr(
                request.profile.model,
                u64::from(response.preprocessing.output_width)
                    * u64::from(response.preprocessing.output_height),
            );
            if response.request_id != request.request_id
                || response.frame_id != request_frame_id
                || response.topology_generation != request_generation
            {
                self.metrics.record_cancelled_stale_request();
                return Err(VisionError::OcrCancelled {
                    reason: "worker response belongs to an older or different request".to_owned(),
                });
            }
            responses.push(response);
        }

        enter_phase(trace, SceneExecutionPhase::MergingScene);
        let options = SceneBuildOptions {
            region_kind: crate::scene::VisualRegionKind::Content,
            projection: policy.projection,
            row: crate::layout::RowConfig::default(),
            base_scene: if full_refresh { None } else { base_scene },
            refresh_regions: if full_refresh {
                Vec::new()
            } else {
                refresh_regions.clone()
            },
        };
        let scene_merge_started_at = Instant::now();
        let mut builder = self.scene_builder.lock().await;
        let scene = builder.build(window, &frame, &responses, &options)?;
        drop(builder);
        let refreshed_nodes = if full_refresh {
            scene.nodes.len()
        } else {
            scene
                .nodes
                .iter()
                .filter(|node| {
                    refresh_regions
                        .iter()
                        .any(|region| node.bbox.intersects(*region))
                })
                .count()
        };
        self.metrics.record_node_merge(
            scene.nodes.len().saturating_sub(refreshed_nodes),
            refreshed_nodes,
        );
        self.metrics
            .record_scene_merge_latency(scene_merge_started_at.elapsed());
        if full_refresh {
            cache.replace(scene.clone());
        } else {
            cache.replace_regions(scene.clone(), &refresh_regions);
        }
        self.metrics.record_scene_built();
        enter_phase(trace, SceneExecutionPhase::Completed);
        Ok(scene)
    }
}

/// 在可选轨迹存在时记录阶段，普通 runtime 调用不分配诊断状态。
fn enter_phase(trace: Option<&SceneExecutionTrace>, phase: SceneExecutionPhase) {
    if let Some(trace) = trace {
        trace.enter(phase);
    }
}
