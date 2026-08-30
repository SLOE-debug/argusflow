use std::sync::Arc;

use argusflow_core::{AutomationError, WindowIdentity};
use argusflow_query::parse_query;

use super::*;
use crate::{
    CapturedFrame, FrameId, MemoryFrameSource, OcrItem, OcrModel, OcrPreprocessingSummary,
    OcrRequestId, OcrResponse, OcrTimingSummary, PhysicalRect, PolygonPoint, QpcTimestamp,
    StaticOcrEngine, TopologyGeneration, VisionWorkerClient, WindowDescriptor,
};

/// 固定返回一个可捕获窗口的测试注册表。
#[derive(Debug)]
struct SingleWindowInventory {
    /// 测试进程唯一的顶层窗口。
    window: WindowDescriptor,
}

impl WindowInventory for SingleWindowInventory {
    fn windows_for_process(&self, process_id: u32) -> Result<Vec<WindowDescriptor>, VisionError> {
        if self.window.identity.process_id == process_id {
            Ok(vec![self.window.clone()])
        } else {
            Ok(Vec::new())
        }
    }
}

#[tokio::test]
async fn small_miss_returns_to_waiter_without_consuming_medium_refresh() {
    let identity = WindowIdentity {
        handle: 67_956,
        process_id: 12_468,
    };
    let frame_source = Arc::new(MemoryFrameSource::new());
    frame_source.insert(identity, vec![frame(identity, 1), frame(identity, 2)]);
    let worker = Arc::new(VisionWorkerClient::new(Arc::new(StaticOcrEngine::new([
        response(2, "文件传输助手"),
    ]))));
    let runtime = VisionRuntime::new(frame_source, worker);
    let inventory = SingleWindowInventory {
        window: WindowDescriptor {
            identity,
            owner_handle: None,
            z_order: 0,
            screen_bounds: PhysicalRect::new(0, 0, 100, 100).expect("window bounds are valid"),
            foreground: true,
        },
    };
    let source = "text(name contains \"网络结果\")";
    let query = parse_query(source).expect("AQL should parse");
    let plan = crate::compile_vision_query(&query).expect("AQL should compile for OCR scene");

    let error = runtime
        .resolve_query(&inventory, identity.process_id, &plan, source, 0.35, None)
        .await
        .expect_err("a complete Small scene without the text should remain a retryable miss");

    assert!(matches!(error, AutomationError::TargetNotFound { .. }));

    let mut forced_refresh = SceneRefreshPolicy::small();
    forced_refresh.force_refresh = true;
    let refreshed = runtime
        .current_app_scene(&inventory, identity.process_id, &forced_refresh, None)
        .await
        .expect("a quiet reused subscription should retain its last complete scene");
    assert_eq!(refreshed.windows[0].scene.frame_id, FrameId::new(2));

    let topology = runtime
        .revalidate_cache(identity, std::time::Duration::from_millis(1))
        .await
        .expect("a quiet stream with unchanged topology should keep a materialized target valid");
    assert_eq!(topology, TopologyGeneration::new(1));
}

/// 创建稳定门控需要的单张测试帧。
fn frame(identity: WindowIdentity, frame_id: u64) -> Arc<CapturedFrame> {
    Arc::new(
        CapturedFrame::from_bgra8(
            FrameId::new(frame_id),
            TopologyGeneration::new(1),
            identity,
            QpcTimestamp::new(frame_id),
            100,
            100,
            96,
            96,
            400,
            vec![0; 100 * 100 * 4],
        )
        .expect("fixture frame should be valid"),
    )
}

/// 创建与第二张稳定帧绑定的一次 Small OCR 响应。
fn response(frame_id: u64, text: &str) -> OcrResponse {
    OcrResponse {
        request_id: OcrRequestId::new(),
        frame_id: FrameId::new(frame_id),
        topology_generation: TopologyGeneration::new(1),
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
        timings: OcrTimingSummary {
            preprocess_elapsed_ms: 0,
            inference_elapsed_ms: 1,
        },
        model_input: None,
        items: vec![OcrItem {
            raw_text: text.to_owned(),
            confidence: 0.99,
            polygon: vec![
                PolygonPoint { x: 10.0, y: 10.0 },
                PolygonPoint { x: 80.0, y: 10.0 },
                PolygonPoint { x: 80.0, y: 30.0 },
                PolygonPoint { x: 10.0, y: 30.0 },
            ],
        }],
    }
}
