//! 每个 HWND 独立维护的 OCR Scene 与短期缓存。

mod cache;
mod index;
mod model;
mod node;
mod observation;

pub use cache::{CacheLookup, CacheMissReason, VisualSceneCache};
pub use index::VisualSceneIndex;
pub use model::{SceneBuildOptions, SceneId, SceneOcrSummary, VisualScene, VisualSceneBuilder};
pub use node::{VisualNode, VisualNodeId, VisualNodeSource};
pub use observation::{FreshRegion, ObservationCoverage, ObservationState};
