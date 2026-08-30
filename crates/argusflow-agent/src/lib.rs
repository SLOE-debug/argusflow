//! 自动化动作的执行上下文、PreparedPlan 契约与运行时路由器。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod backend;
mod context;
mod evidence;
mod evidence_sink;
mod plan;
mod prepared_plan;
mod router;
mod visual;
mod visual_materialization;

pub use backend::ActionBackend;
pub use context::{
    AccessibilityContext, BrowserSessionContext, ContextFitness, ExecutionContext,
    ExecutionContextProvider, ProcessContext, StaticExecutionContext, VisualCacheContext,
    WindowContext,
};
pub use evidence::{
    EvidenceArtifact, EvidenceArtifactData, EvidenceArtifactKind, EvidenceBudget, EvidenceBundle,
    EvidenceCaptureError, EvidenceCapturePolicy, EvidenceCaptureRequest, EvidenceOutcome,
    EvidenceRecord, EvidenceRetentionPolicy, EvidenceTrigger, PreparedDiagnostics,
};
pub use evidence_sink::{
    DiscardEvidenceSink, EvidenceReference, EvidenceSettings, EvidenceSink, EvidenceSinkError,
    FileSystemEvidenceSink, InMemoryEvidenceSink,
};
pub use plan::{
    PlanExplain, PlanRejection, PlanStepExplain, PlanStepKind, PlanningReport, RuntimeAvailability,
};
pub use prepared_plan::{PreparedCandidate, PreparedExecution, PreparedPlan};
pub use router::{ActionRouter, ROUTE_TIE_BREAK_ORDER};
pub use visual::{
    MaterializedTarget, MaterializedTargetValidator, PreparedTargetMaterialization,
    PreparedTargetMaterializer, SharedTargetMaterializer, VisualBaseline,
    VisualMaterializationPlan, VisualMaterializationStage, VisualTargetBounds,
    VisualVerificationProvider, VisualVerificationResult,
};
