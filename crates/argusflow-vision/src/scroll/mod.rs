//! 以视觉事实驱动的安全滚动会话。

mod controller;
mod displacement;
mod end;
mod history;
mod model;
mod session;

pub use controller::{ScrollController, ScrollControllerConfig, ScrollControllerOutcome};
pub use displacement::{
    DisplacementConfig, DisplacementEstimate, DisplacementMethod, estimate_displacement,
    estimate_displacement_with_config,
};
pub use end::{ScrollEndConfig, ScrollEndDetector};
pub use history::{HistoryAppend, ScrollDocumentHistory};
pub use model::{
    AnchorMatchEvidence, PageItem, PageSnapshot, ScrollAnchor, ScrollCalibration, ScrollDirection,
    ScrollRegion, WheelSteps, match_anchors,
};
pub use session::{AcceptedPage, PageTransition, ScrollSession};
