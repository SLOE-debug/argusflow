use crate::*;
use argusflow_core::*;
use std::sync::Arc;

#[tokio::test]
async fn reverse_hit_test_uses_capture_origin_and_existing_ocr_scene() {
    let window = WindowIdentity {
        handle: 42,
        process_id: 7,
    };
    let capture = Arc::new(MemoryFrameSource::new());
    let frames = [1, 2]
        .into_iter()
        .map(|id| {
            Arc::new(
                CapturedFrame::from_bgra8(
                    FrameId::new(id),
                    TopologyGeneration::new(1),
                    window,
                    QpcTimestamp::new(id),
                    100,
                    100,
                    144,
                    144,
                    400,
                    vec![0; 40_000],
                )
                .unwrap()
                .with_screen_origin(ScreenPoint { x: -900, y: 200 }),
            )
        })
        .collect();
    capture.insert(window, frames);
    let response = OcrResponse {
        request_id: OcrRequestId::new(),
        frame_id: FrameId::new(2),
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
            raw_text: "确定".into(),
            confidence: 0.99,
            polygon: vec![
                PolygonPoint { x: 10.0, y: 10.0 },
                PolygonPoint { x: 80.0, y: 10.0 },
                PolygonPoint { x: 80.0, y: 30.0 },
                PolygonPoint { x: 10.0, y: 30.0 },
            ],
        }],
    };
    let runtime = VisionRuntime::new(
        capture,
        Arc::new(VisionWorkerClient::new(Arc::new(StaticOcrEngine::new([
            response,
        ])))),
    );
    let context = InspectionContext {
        window,
        executable_path: None,
        title: "App".into(),
        class_name: "CustomCanvas".into(),
        bounds: InspectionRect {
            x: -950.0,
            y: 150.0,
            width: 200.0,
            height: 200.0,
        },
        browser_viewport: None,
        dpi: 144,
        has_keyboard_focus: true,
    };
    let entity = runtime
        .inspect(
            &context,
            InspectionProbe::Point(ScreenPoint { x: -880, y: 220 }),
        )
        .await
        .unwrap();
    assert_eq!(entity.semantics.name.as_deref(), Some("确定"));
    assert_eq!(entity.bounds.x, -890.0);
    assert_eq!(entity.bounds.y, 210.0);
    assert_eq!(entity.sensitivity, FieldSensitivity::Unknown);
    assert_eq!(
        runtime.inspect(&context, InspectionProbe::Focus).await,
        Err(InspectionFailure::NoElement)
    );
    assert_eq!(
        runtime
            .inspect(
                &context,
                InspectionProbe::Point(ScreenPoint { x: -805, y: 295 })
            )
            .await,
        Err(InspectionFailure::NoElement)
    );
}
