//! VisualScene、region 和 scene cache。

mod cache;
mod delta;
mod model;
mod node;
mod observation;
mod region;

pub use cache::{CacheLookup, CacheMissReason, VisualSceneCache};
pub use delta::{VisualNodeChange, VisualSceneDelta, diff_scenes};
pub use model::{SceneBuildOptions, SceneId, SceneOcrSummary, VisualScene, VisualSceneBuilder};
pub use node::{RoleHint, VisualNode, VisualNodeId, VisualNodeSource};
pub use observation::{FreshRegion, ObservationCoverage, ObservationState};
pub use region::{VisualRegion, VisualRegionId, VisualRegionKind};
