//! 相邻动作之间一次性交接已经严格解析的视觉目标，避免瞬态窗口上的重复 OCR。

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use argusflow_core::{RunTraceContext, UiQuery};
use argusflow_query::canonicalize_query;
use uuid::Uuid;

use super::VisionRuntime;
use crate::ResolvedTextTarget;

/// 相邻节点允许消费已解析目标的最大时间窗口。
const TARGET_HANDOFF_TTL: Duration = Duration::from_millis(500);

/// 已完成参数绑定的 AQL 稳定身份，防止调用方用原始源码或弱字符串误配目标。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedTargetHandoffKey(String);

impl ResolvedTargetHandoffKey {
    /// 从 Runtime 已冻结的查询生成规范化身份。
    pub fn from_query(query: &UiQuery) -> Self {
        Self(canonicalize_query(query))
    }
}

/// 一次流程中某个进程、某条已绑定查询的交接槽位。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TargetHandoffSlot {
    /// 工作流本次运行身份，隔离并发运行。
    run_id: Uuid,
    /// 已冻结应用会话对应的进程。
    process_id: u32,
    /// 参数绑定后的查询身份。
    query: ResolvedTargetHandoffKey,
}

/// 尚未被后续动作消费的唯一视觉目标事实。
#[derive(Debug)]
struct TargetHandoffEntry {
    /// 产生事实的节点，禁止同一节点回读自己的临时状态。
    source_node_id: String,
    /// 事实完成严格唯一解析的单调时间。
    resolved_at: Instant,
    /// 与原始 Scene、窗口和 OCR bbox 绑定的目标。
    target: ResolvedTextTarget,
}

/// 短生命周期、一次性消费的运行内视觉目标交接表。
#[derive(Debug, Default)]
pub(super) struct ResolvedTargetHandoffStore {
    /// 每个 run/process/query 最多保留最近一个事实，避免无界累积。
    entries: HashMap<TargetHandoffSlot, TargetHandoffEntry>,
}

impl ResolvedTargetHandoffStore {
    /// 发布刚刚通过严格唯一查询的目标，并清理所有过期事实。
    fn publish(
        &mut self,
        context: &RunTraceContext,
        process_id: u32,
        query: &ResolvedTargetHandoffKey,
        target: &ResolvedTextTarget,
        now: Instant,
    ) {
        self.retain_fresh(now);
        self.entries.insert(
            slot(context.run_id, process_id, query),
            TargetHandoffEntry {
                source_node_id: context.node_id.clone(),
                resolved_at: now,
                target: target.clone(),
            },
        );
    }

    /// 一次性取得相邻节点可复用的目标；任何语义或质量不匹配都回到正常 OCR。
    fn take(
        &mut self,
        context: &RunTraceContext,
        process_id: u32,
        query: &ResolvedTargetHandoffKey,
        minimum_confidence: f32,
        now: Instant,
    ) -> Option<ResolvedTextTarget> {
        self.retain_fresh(now);
        let entry = self
            .entries
            .remove(&slot(context.run_id, process_id, query))?;
        if entry.source_node_id == context.node_id
            || entry.target.node.confidence < minimum_confidence
        {
            return None;
        }
        Some(entry.target)
    }

    /// 移除超过相邻动作窗口的事实，确保慢路径不会误用陈旧弹窗坐标。
    fn retain_fresh(&mut self, now: Instant) {
        self.entries.retain(|_, entry| {
            now.checked_duration_since(entry.resolved_at)
                .is_some_and(|age| age <= TARGET_HANDOFF_TTL)
        });
    }
}

impl VisionRuntime {
    /// 为同一次运行中的紧邻输入动作发布一个可复验目标事实。
    pub async fn publish_resolved_target_handoff(
        &self,
        context: &RunTraceContext,
        process_id: u32,
        query: &ResolvedTargetHandoffKey,
        target: &ResolvedTextTarget,
    ) {
        self.target_handoffs.lock().await.publish(
            context,
            process_id,
            query,
            target,
            Instant::now(),
        );
    }

    /// 一次性消费同一运行内的最近目标；调用方必须在输入提交点继续复验新鲜度。
    pub async fn take_resolved_target_handoff(
        &self,
        context: &RunTraceContext,
        process_id: u32,
        query: &ResolvedTargetHandoffKey,
        minimum_confidence: f32,
    ) -> Option<ResolvedTextTarget> {
        self.target_handoffs.lock().await.take(
            context,
            process_id,
            query,
            minimum_confidence,
            Instant::now(),
        )
    }
}

/// 构造不含节点身份的稳定槽位键。
fn slot(run_id: Uuid, process_id: u32, query: &ResolvedTargetHandoffKey) -> TargetHandoffSlot {
    TargetHandoffSlot {
        run_id,
        process_id,
        query: query.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argusflow_core::{ScreenPoint, WindowIdentity};
    use argusflow_query::parse_query;

    use super::*;
    use crate::{
        FrameId, OcrModel, PhysicalRect, PolygonPoint, SceneId, SceneOcrSummary,
        TopologyGeneration, VisualNode, VisualNodeSource, VisualScene, VisualSceneIndex,
        WindowDescriptor,
    };

    #[test]
    fn handoff_is_scoped_one_shot_and_preserves_click_confidence() {
        let mut store = ResolvedTargetHandoffStore::default();
        let run_id = Uuid::new_v4();
        let source = context(run_id, "read");
        let consumer = context(run_id, "click");
        let query = query_key();
        let target = target(0.92);
        let now = Instant::now();

        store.publish(&source, 12_468, &query, &target, now);

        let consumed = store
            .take(&consumer, 12_468, &query, 0.80, now)
            .expect("adjacent click should consume the exact resolved target");
        assert_eq!(consumed.node.raw_text, "崽崽");
        assert!(
            store.take(&consumer, 12_468, &query, 0.80, now).is_none(),
            "handoff must not be replayed"
        );
    }

    #[test]
    fn handoff_rejects_expired_or_lower_confidence_targets() {
        let run_id = Uuid::new_v4();
        let source = context(run_id, "read");
        let consumer = context(run_id, "click");
        let query = query_key();
        let now = Instant::now();
        let mut store = ResolvedTargetHandoffStore::default();
        store.publish(&source, 12_468, &query, &target(0.70), now);
        assert!(store.take(&consumer, 12_468, &query, 0.80, now).is_none());

        store.publish(&source, 12_468, &query, &target(0.92), now);
        assert!(
            store
                .take(
                    &consumer,
                    12_468,
                    &query,
                    0.80,
                    now + TARGET_HANDOFF_TTL + Duration::from_millis(1),
                )
                .is_none()
        );
    }

    /// 创建测试用 Run/Node 身份。
    fn context(run_id: Uuid, node_id: &str) -> RunTraceContext {
        RunTraceContext {
            run_id,
            node_id: node_id.to_owned(),
            node_sequence: 0,
        }
    }

    /// 创建已完成变量绑定的联系人查询身份。
    fn query_key() -> ResolvedTargetHandoffKey {
        let query = parse_query(
            r#"nearest(anchor=text(name="最常使用"),target=text(name="崽崽"),direction=below,index=1)"#,
        )
        .expect("fixture AQL should parse");
        ResolvedTargetHandoffKey::from_query(&query)
    }

    /// 创建携带窗口、Scene 和 bbox 的最小已解析目标。
    fn target(confidence: f32) -> ResolvedTextTarget {
        let identity = WindowIdentity {
            handle: 592_116,
            process_id: 12_468,
        };
        let polygon = vec![
            PolygonPoint { x: 10.0, y: 30.0 },
            PolygonPoint { x: 60.0, y: 30.0 },
            PolygonPoint { x: 60.0, y: 50.0 },
            PolygonPoint { x: 10.0, y: 50.0 },
        ];
        let node = VisualNode::from_ocr(
            SceneId::new(1),
            "崽崽".to_owned(),
            confidence,
            polygon,
            VisualNodeSource::OcrSmall,
        )
        .expect("fixture node should be valid");
        let scene = Arc::new(VisualScene {
            scene_id: SceneId::new(1),
            frame_id: FrameId::new(1),
            topology_generation: TopologyGeneration::new(1),
            window: identity,
            viewport: PhysicalRect::new(0, 0, 100, 100).expect("valid viewport"),
            viewport_origin: ScreenPoint { x: 100, y: 200 },
            nodes: vec![node.clone()],
            index: VisualSceneIndex::build(std::slice::from_ref(&node)),
            ocr: SceneOcrSummary {
                models: vec![OcrModel::PpOcrV6Small],
                request_count: 1,
                item_count: 1,
                elapsed_ms: 1,
                enhanced_request_count: 0,
                max_scale_milli: 1_000,
            },
            built_at_unix_ms: 1,
        });
        ResolvedTextTarget {
            window: WindowDescriptor {
                identity,
                owner_handle: Some(68_116),
                z_order: 0,
                screen_bounds: PhysicalRect::new(100, 200, 100, 100).expect("valid screen bounds"),
                foreground: false,
            },
            scene,
            node,
        }
    }
}
