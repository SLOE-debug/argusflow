//! 相对操作前基准检测变化，相对候选像素确认稳定，不依赖帧是否持续产生。
use super::{
    model::{Analysis, Decision},
    regions::changed,
};
use argusflow_windows::VideoThumbnail;
#[cfg(test)]
#[path = "../../../../../tests/argusflow-desktop/unit/recorder/video_analysis.rs"]
mod tests;
pub(super) struct Selection {
    baseline: VideoThumbnail,
    reference: VideoThumbnail,
    candidate: u64,
    changed_at: i64,
    regions: Vec<[u32; 4]>,
    compared: usize,
}
impl Selection {
    pub fn new(baseline: VideoThumbnail, sequence: u64, at: i64) -> Self {
        let reference = VideoThumbnail {
            width: baseline.width,
            height: baseline.height,
            pixels: baseline.pixels.clone(),
        };
        Self {
            baseline,
            reference,
            candidate: sequence,
            changed_at: at,
            regions: vec![],
            compared: 0,
        }
    }
    pub fn push(
        &mut self,
        thumbnail: VideoThumbnail,
        sequence: u64,
        at: i64,
        end: i64,
        earliest: i64,
        frequency: u64,
    ) -> Result<bool, String> {
        if thumbnail.width != self.baseline.width
            || thumbnail.height != self.baseline.height
            || thumbnail.pixels.len() != self.baseline.pixels.len()
        {
            return Err("视频分析尺寸发生变化".into());
        }
        self.compared += 1;
        if !changed(&self.reference, &thumbnail)?.is_empty() {
            self.regions = changed(&self.baseline, &thumbnail)?;
            self.reference = thumbnail;
            self.candidate = sequence;
            self.changed_at = at;
        }
        Ok(!self.regions.is_empty()
            && i128::from(end) - i128::from(self.changed_at.max(earliest))
                >= i128::from(frequency) / 5)
    }
    pub fn decision(self, settled: bool, size: [u32; 2]) -> Decision {
        let regions = self
            .regions
            .into_iter()
            .map(|[x, y, w, h]| {
                let x0 = x * size[0] / self.baseline.width;
                let y0 = y * size[1] / self.baseline.height;
                let x1 = (x + w) * size[0] / self.baseline.width;
                let y1 = (y + h) * size[1] / self.baseline.height;
                [x0, y0, x1 - x0, y1 - y0]
            })
            .collect::<Vec<_>>();
        Decision {
            sequence: self.candidate,
            analysis: Analysis {
                status: if regions.is_empty() {
                    "no_change"
                } else if settled {
                    "settled"
                } else {
                    "changing"
                }
                .into(),
                compared_frames: self.compared,
                regions,
            },
        }
    }
}
