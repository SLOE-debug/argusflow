//! ArgusFlow 的窗口视觉感知、OCR 与可解释视觉查询运行时。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod app_scene;
mod backend;
mod diagnostics;
mod diff;
mod error;
mod frame;
mod image;
mod metrics;
mod ocr;
mod projection;
mod query;
mod refresh;
mod region;
mod runtime;
mod scene;
mod scope;
mod source;
mod stability;
mod trace;
mod window;
mod worker;

pub use app_scene::{AppNodeRef, AppScene, AppSceneSummary, AppWindowScene, ResolvedTextTarget};
pub use backend::VisionBackend;
pub use diagnostics::encode_bgra_as_bmp;
pub use diff::{DiffConfig, DirtyMap, DirtyRegion, DirtyRegionReason, compute_dirty_map};
pub use error::VisionError;
pub use frame::{
    CoordinateSpace, FrameId, PhysicalRect, PixelFormat, QpcTimestamp, TopologyGeneration,
};
pub use image::{CapturedFrame, PixelImage};
pub use metrics::{VisionMetrics, VisionMetricsSnapshot};
pub use ocr::{
    OcrDiagnosticImageEncoding, OcrEngine, OcrImagePreprocessing, OcrItem, OcrModel,
    OcrModelInputArtifact, OcrOptions, OcrPreprocessingSummary, OcrProfile, OcrRequest,
    OcrRequestId, OcrResponse, OcrTimingSummary, PolygonPoint, normalize_text,
};
pub use projection::{SceneNodeProjection, SceneProjection, project_app_scene};
pub use query::{
    VisionQueryCompileError, VisionQueryMetrics, VisionQueryPlan, VisionQueryResult, VisualMatch,
    VisualQueryCandidateSummary, VisualQueryReport, compile_vision_query, evaluate_app_query,
    evaluate_vision_query, evaluate_visual_query, matching_app_nodes, matching_nodes,
    require_unique,
};
pub use refresh::{RefreshPlan, RefreshReason, choose_refresh_plan};
pub use region::normalized_region_to_physical;
pub use runtime::{SceneRefreshPolicy, VisionHealth, VisionRuntime, VisualSceneService};
pub use scene::{
    CacheLookup, CacheMissReason, FreshRegion, ObservationCoverage, ObservationState,
    SceneBuildOptions, SceneId, SceneOcrSummary, VisualNode, VisualNodeId, VisualNodeSource,
    VisualScene, VisualSceneBuilder, VisualSceneCache, VisualSceneIndex,
};
pub use source::{CapturePolicy, FrameSubscription, MemoryFrameSource, WindowFrameSource};
pub use stability::{StabilityConfig, StabilityState, StableFrameGate};
pub use trace::{VisionSelectionOutcome, VisionTraceSink};
pub use window::{WindowDescriptor, WindowInventory};
#[cfg(target_os = "windows")]
pub use worker::NamedPipeOcrEngine;
pub use worker::{
    MAX_PIXEL_BODY_BYTES, SharedMemoryPixels, StaticOcrEngine, UnavailableOcrEngine,
    VISION_PROTOCOL_VERSION, VisionWorkerClient, WorkerBinaryArtifact, WorkerBinaryArtifactKind,
    WorkerCommand, WorkerDiagnosticsRequest, WorkerError, WorkerHealth, WorkerLifecycle,
    WorkerModelInfo, WorkerOcrRequest, WorkerProtocolEnvelope, WorkerResponse,
};
