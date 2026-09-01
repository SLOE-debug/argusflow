//! 单次工作流运行的持久化诊断事实与只读历史查询。

mod files;
mod index;
mod model;
mod session_helpers;
mod store;
#[cfg(test)]
mod store_tests;

pub use model::{
    RUN_SCENE_PROJECTION_SCHEMA_VERSION, ResolvedInputField, ResolvedInputSource,
    ResolvedNodeInputs, RunArtifactKind, RunArtifactSummary, RunDetails, RunManifest,
    RunNodeOutputs, RunNodeTrace, RunPixelPoint, RunPixelRect, RunPresentationSnapshot,
    RunSceneNodeProjection, RunSceneNodeRef, RunSceneProjection, RunSceneWindowProjection,
    RunStatus, RunTraceEvent, RunTraceLevel, RunVisualQueryMetrics, RunVisualQueryTrace,
    RunVisualSelectionOutcome,
};
pub use store::{FileRunTraceStore, RunTraceSession, RunTraceStore};
