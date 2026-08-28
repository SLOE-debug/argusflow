//! VisualScene、region 和 scene cache。

mod cache;
mod delta;
mod model;
mod node;
mod region;

pub use cache::{CacheLookup, CacheMissReason, VisualSceneCache};
pub use delta::{VisualNodeChange, VisualSceneDelta, diff_scenes};
pub use model::{SceneBuildOptions, SceneId, VisualScene, VisualSceneBuilder};
pub use node::{RoleHint, VisualNode, VisualNodeId, VisualNodeSource};
pub use region::{VisualRegion, VisualRegionId, VisualRegionKind};
