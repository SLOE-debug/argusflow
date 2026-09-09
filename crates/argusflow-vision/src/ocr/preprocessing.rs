//! PP-OCRv6 的 BGR 预处理和文本四边形透视裁剪。
use crate::OcrConfig;
use crate::OcrError as Failure;
use argusflow_core::{FailureKind, ImagePoint, Operation};
use image::{
    Rgb, RgbImage,
    imageops::{self, FilterType},
};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use ndarray::Array4;

pub(crate) fn detection(
    image: &RgbImage,
    config: &OcrConfig,
    operation: &Operation,
) -> Result<Array4<f32>, Failure> {
    let (width, height) = image.dimensions();
    // 与官方 min-side 策略一致，极小图片先补足 32 像素。
    let padded = if width + height < 64 {
        let mut padded = RgbImage::new(width.max(32), height.max(32));
        imageops::replace(&mut padded, image, 0, 0);
        Some(padded)
    } else {
        None
    };
    let source = padded.as_ref().unwrap_or(image);
    let ratio =
        (config.detection_min_side as f64 / source.width().min(source.height()) as f64).max(1.0);
    let ratio =
        ratio.min(config.detection_max_side as f64 / source.width().max(source.height()) as f64);
    let align =
        |value: u32| (((value as f64 * ratio / 32.0).round_ties_even() as u32) * 32).max(32);
    let resized = imageops::resize(
        source,
        align(source.width()),
        align(source.height()),
        FilterType::Triangle,
    );
    let mut tensor = Array4::zeros((1, 3, resized.height() as usize, resized.width() as usize));
    let mean = [0.485, 0.456, 0.406];
    let std = [0.229, 0.224, 0.225];
    for (y, row) in resized.enumerate_rows() {
        operation.check("det_preprocess")?;
        for (x, _, pixel) in row {
            for channel in 0..3 {
                tensor[[0, channel, y as usize, x as usize]] =
                    (pixel[2 - channel] as f32 / 255.0 - mean[channel]) / std[channel];
            }
        }
    }
    Ok(tensor)
}

pub(crate) fn crop(
    image: &RgbImage,
    polygon: &[ImagePoint; 4],
    operation: &Operation,
) -> Result<RgbImage, Failure> {
    operation.check("text_crop")?;
    let distance =
        |a: ImagePoint, b: ImagePoint| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
    let width = distance(polygon[0], polygon[1])
        .max(distance(polygon[3], polygon[2]))
        .round()
        .max(1.0) as u32;
    let height = distance(polygon[0], polygon[3])
        .max(distance(polygon[1], polygon[2]))
        .round()
        .max(1.0) as u32;
    if u64::from(width) * u64::from(height) > 16_000_000 {
        return Err(Failure::new(
            FailureKind::ResourceLimit,
            "text_crop",
            "文本区域像素数超限",
        ));
    }
    let from = polygon.map(|point| (point.x, point.y));
    let to = [
        (0.0, 0.0),
        ((width - 1) as f32, 0.0),
        ((width - 1) as f32, (height - 1) as f32),
        (0.0, (height - 1) as f32),
    ];
    let projection = Projection::from_control_points(from, to).ok_or_else(|| {
        Failure::new(
            FailureKind::Protocol,
            "text_crop",
            "文本四边形无法生成透视变换",
        )
    })?;
    let mut cropped = RgbImage::new(width, height);
    warp_into(
        image,
        &projection,
        Interpolation::Bilinear,
        Rgb([255, 255, 255]),
        &mut cropped,
    );
    // 与官方 crop 策略一致：竖长文本区域逆时针旋转，不添加方向模型。
    Ok(if height as f32 / width as f32 >= 1.5 {
        imageops::rotate270(&cropped)
    } else {
        cropped
    })
}

pub(crate) fn recognition(image: &RgbImage, operation: &Operation) -> Result<Array4<f32>, Failure> {
    let natural_width = (48.0 * image.width() as f64 / image.height() as f64).ceil() as u32;
    let width = natural_width.clamp(1, 3200);
    let tensor_width = width.max(320);
    let resized = imageops::resize(image, width, 48, FilterType::Triangle);
    let mut tensor = Array4::<f32>::zeros((1, 3, 48, tensor_width as usize));
    for (y, row) in resized.enumerate_rows() {
        operation.check("rec_preprocess")?;
        for (x, _, pixel) in row {
            for channel in 0..3 {
                tensor[[0, channel, y as usize, x as usize]] =
                    pixel[2 - channel] as f32 / 127.5 - 1.0;
            }
        }
    }
    Ok(tensor)
}
