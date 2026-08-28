//! ArgusFlow 的窗口视觉感知、OCR 与可解释视觉查询运行时。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod backend;
mod diff;
mod error;
mod evidence;
mod frame;
mod image;
mod layout;
mod metrics;
mod ocr;
mod projection;
mod query;
mod refresh;
mod region;
mod runtime;
mod scene;
mod scope;
mod scroll;
mod source;
mod stability;
mod verification;
mod worker;

pub use backend::VisionBackend;
pub use diff::{DiffConfig, DirtyMap, DirtyRegion, DirtyRegionReason, compute_dirty_map};
pub use error::VisionError;
pub use evidence::VisionPreparedDiagnostics;
pub use frame::{
    CoordinateSpace, FrameId, PhysicalRect, PixelFormat, QpcTimestamp, TopologyGeneration,
};
pub use image::{CapturedFrame, PixelImage};
pub use layout::{
    RowConfig, VisualLine, VisualLineId, VisualRow, VisualRowId, cluster_lines, cluster_rows,
};
pub use metrics::{VisionMetrics, VisionMetricsSnapshot};
pub use ocr::{
    OcrEngine, OcrItem, OcrModel, OcrOptions, OcrProfile, OcrRequest, OcrRequestId, OcrResponse,
    OcrSource, PolygonPoint, normalize_text,
};
pub use projection::{ProjectionOptions, compact_text, spatial_text};
pub use query::{
    VisualCandidate, VisualMatch, evaluate_visual_query, fuzzy_candidates, matching_nodes,
};
pub use refresh::{RefreshPlan, RefreshReason, choose_refresh_plan};
pub use runtime::{SceneRefreshPolicy, VisionHealth, VisionRuntime, VisualSceneService};
pub use scene::{
    CacheLookup, CacheMissReason, RoleHint, SceneBuildOptions, SceneId, VisualNode,
    VisualNodeChange, VisualNodeId, VisualNodeSource, VisualRegion, VisualRegionId,
    VisualRegionKind, VisualScene, VisualSceneDelta, diff_scenes,
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
