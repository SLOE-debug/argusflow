use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Analysis {
    pub status: String,
    pub compared_frames: usize,
    pub regions: Vec<[u32; 4]>,
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Decision {
    pub sequence: u64,
    pub analysis: Analysis,
}
