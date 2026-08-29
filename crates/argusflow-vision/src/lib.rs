//! ArgusFlow 的窗口视觉感知、OCR 与可解释视觉查询运行时。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod backend;
mod diagnostics;
mod diff;
mod error;
mod evidence;
mod frame;
mod image;
mod index;
mod layout;
mod metrics;
mod ocr;
mod projection;
mod query;
mod refresh;
mod region;
mod runtime;
mod scene;
mod scene_execution;
mod scope;
mod scroll;
mod source;
mod stability;
mod verification;
mod worker;

pub use backend::VisionBackend;
pub use diff::{DiffConfig, DirtyMap, DirtyRegion, DirtyRegionReason, compute_dirty_map};
pub use error::{SceneExecutionPhase, VisionError};
pub use evidence::VisionPreparedDiagnostics;
pub use frame::{
    CoordinateSpace, FrameId, PhysicalRect, PixelFormat, QpcTimestamp, TopologyGeneration,
};
pub use image::{CapturedFrame, PixelImage};
pub use index::{
    VisualSceneIndex, VisualSceneSnapshot, center_distance_normalized, direction_matches,
    edge_gap_normalized,
};
pub use layout::{
    RowConfig, VisualLine, VisualLineId, VisualRow, VisualRowId, cluster_lines, cluster_rows,
};
pub use metrics::{VisionMetrics, VisionMetricsSnapshot};
pub use ocr::{
    OcrEngine, OcrImagePreprocessing, OcrItem, OcrModel, OcrOptions, OcrPreprocessingSummary,
    OcrProfile, OcrRequest, OcrRequestId, OcrResponse, OcrSource, PolygonPoint, normalize_text,
};
pub use projection::{ProjectionOptions, compact_text, spatial_text};
pub use query::{
    PreparedVisionQuery, VisionPlanExpr, VisionQueryCompileError, VisionQueryExecutionError,
    VisionQueryPlan, VisionQueryResult, VisionTextPredicate, VisualCandidate, VisualMatch,
    VisualQueryCandidateSummary, VisualQueryReport, compile_vision_query, evaluate_visual_query,
    execute_unique_vision_query, execute_vision_query, fuzzy_candidates, matching_nodes,
};
pub use refresh::{RefreshPlan, RefreshReason, choose_refresh_plan};
pub use region::normalized_region_to_physical;
pub use runtime::{SceneRefreshPolicy, VisionHealth, VisionRuntime, VisualSceneService};
pub use scene::{
    CacheLookup, CacheMissReason, FreshRegion, ObservationCoverage, ObservationState, RoleHint,
    SceneBuildOptions, SceneId, SceneOcrSummary, VisualNode, VisualNodeChange, VisualNodeId,
    VisualNodeSource, VisualRegion, VisualRegionId, VisualRegionKind, VisualScene,
    VisualSceneBuilder, VisualSceneDelta, diff_scenes,
};
// Windows 输入层仍以该强类型值表达滚轮批次；其余滚动编排保持 crate 内部实现。
pub use scroll::WheelSteps;
pub use source::{CapturePolicy, FrameSubscription, MemoryFrameSource, WindowFrameSource};
pub use stability::{
    StabilityConfig, StabilityState, StableFrameGate, TemporalNoiseConfig, TemporalNoiseMask,
};
pub use verification::{VerificationOutcome, VisualCondition, evaluate_visual_condition};
#[cfg(target_os = "windows")]
pub use worker::NamedPipeOcrEngine;
pub use worker::{
    MAX_PIXEL_BODY_BYTES, PixelTransport, StaticOcrEngine, UnavailableOcrEngine,
    VISION_PROTOCOL_VERSION, VisionWorkerClient, WorkerCommand, WorkerError, WorkerHealth,
    WorkerLifecycle, WorkerModelInfo, WorkerOcrRequest, WorkerProtocolEnvelope, WorkerResponse,
};
