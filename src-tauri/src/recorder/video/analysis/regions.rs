//! 录制器的微小变化过滤策略；像素差分和连通分量由公共图像模块实现。
use argusflow_core::{Operation, OperationOptions};
use argusflow_image::{DifferencePolicy, ImageView, changed_regions};
use argusflow_windows::VideoThumbnail;

pub(super) fn changed(a: &VideoThumbnail, b: &VideoThumbnail) -> Result<Vec<[u32; 4]>, String> {
    let compare = || {
        changed_regions(
            ImageView::triples(a.width, a.height, a.pixels.as_flattened())?,
            ImageView::triples(b.width, b.height, b.pixels.as_flattened())?,
            DifferencePolicy {
                threshold: 12,
                min_pixels: 6,
                min_width: 2,
                min_height: 2,
            },
            &Operation::new(OperationOptions::default()),
        )
    };
    compare()
        .map(|regions| {
            regions
                .into_iter()
                .take(12)
                .map(|region| {
                    let bounds = region.bounds;
                    [bounds.x(), bounds.y(), bounds.width(), bounds.height()]
                })
                .collect()
        })
        .map_err(|error| error.to_string())
}
