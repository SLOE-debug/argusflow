//! 图片输入验证与有界解码。
use crate::OcrConfig;
use crate::OcrError as Failure;
use argusflow_core::{FailureKind, Operation};
use image::{ImageReader, Limits, Rgb, RgbImage};
use std::{
    io::{Cursor, Read},
    path::PathBuf,
};

/// 原始像素的通道布局，所有通道均为 u8。
#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    /// RGB 三通道。
    Rgb,
    /// BGR 三通道。
    Bgr,
    /// RGBA 四通道，透明部分合成到白底。
    Rgba,
    /// BGRA 四通道，透明部分合成到白底。
    Bgra,
    /// BGR 与填充字节；第四通道不代表透明度。
    Bgrx,
}

/// 一次独立的图片识别输入；不含截图采集或跨帧状态。
#[derive(Debug)]
pub enum ImageInput {
    /// 采样模块交付的不可变共享像素，保留原始预算租约。
    Shared(argusflow_capture_contracts::PixelImage),
    /// PNG、JPEG、BMP 或 WebP 图片路径。
    Path(PathBuf),
    /// 支持格式的编码图片字节。
    Encoded(Vec<u8>),
    /// 带明确步长的原始像素。
    Pixels {
        /// 图片宽度。
        width: u32,
        /// 图片高度。
        height: u32,
        /// 每行字节数，允许末尾填充。
        stride: usize,
        /// 通道布局。
        format: PixelFormat,
        /// stride * height 个字节。
        bytes: Vec<u8>,
    },
}

impl ImageInput {
    pub(crate) fn validate(&self, config: &OcrConfig) -> Result<(), Failure> {
        match self {
            Self::Shared(image) => dimensions(image.width(), image.height(), config),
            Self::Path(_) => Ok(()),
            Self::Encoded(bytes) => encoded_size(bytes.len(), config),
            Self::Pixels {
                width,
                height,
                stride,
                format,
                bytes,
            } => {
                dimensions(*width, *height, config)?;
                let channels = match format {
                    PixelFormat::Rgb | PixelFormat::Bgr => 3,
                    PixelFormat::Rgba | PixelFormat::Bgra | PixelFormat::Bgrx => 4,
                };
                if *stride < *width as usize * channels
                    || stride.checked_mul(*height as usize) != Some(bytes.len())
                    || bytes.len() > config.max_pixels as usize * 4
                {
                    return Err(invalid("像素步长、长度或通道布局不匹配"));
                }
                Ok(())
            }
        }
    }
    pub(crate) fn decode(
        self,
        config: &OcrConfig,
        operation: &Operation,
    ) -> Result<RgbImage, Failure> {
        self.validate(config)?;
        operation.check("image_decode")?;
        match self {
            Self::Shared(image) => {
                let format = match image.format() {
                    argusflow_capture_contracts::PixelFormat::Bgrx8 => PixelFormat::Bgrx,
                    argusflow_capture_contracts::PixelFormat::Bgra8 => PixelFormat::Bgra,
                    argusflow_capture_contracts::PixelFormat::Rgba8 => PixelFormat::Rgba,
                };
                decode_pixels(
                    image.width(),
                    image.height(),
                    image.stride(),
                    format,
                    image.bytes(),
                    operation,
                )
            }
            Self::Path(path) => {
                let file = std::fs::File::open(path)
                    .map_err(|error| invalid("无法打开图片文件").with_source(error))?;
                let mut bytes = Vec::new();
                file.take(config.max_encoded_bytes as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| invalid("无法读取图片文件").with_source(error))?;
                encoded_size(bytes.len(), config)?;
                decode_bytes(bytes, config, operation)
            }
            Self::Encoded(bytes) => decode_bytes(bytes, config, operation),
            Self::Pixels {
                width,
                height,
                stride,
                format,
                bytes,
            } => decode_pixels(width, height, stride, format, &bytes, operation),
        }
    }
}

fn decode_pixels(
    width: u32,
    height: u32,
    stride: usize,
    format: PixelFormat,
    bytes: &[u8],
    operation: &Operation,
) -> Result<RgbImage, Failure> {
    let channels = match format {
        PixelFormat::Rgb | PixelFormat::Bgr => 3,
        _ => 4,
    };
    let mut image = RgbImage::new(width, height);
    for (y, row) in bytes.chunks_exact(stride).enumerate() {
        operation.check("pixel_decode")?;
        for (x, pixel) in row[..width as usize * channels]
            .chunks_exact(channels)
            .enumerate()
        {
            let (red, blue) = match format {
                PixelFormat::Rgb | PixelFormat::Rgba => (pixel[0], pixel[2]),
                PixelFormat::Bgr | PixelFormat::Bgra | PixelFormat::Bgrx => (pixel[2], pixel[0]),
            };
            let alpha = if matches!(format, PixelFormat::Rgba | PixelFormat::Bgra) {
                pixel[3]
            } else {
                255
            };
            image.put_pixel(
                x as u32,
                y as u32,
                Rgb([
                    composite(red, alpha),
                    composite(pixel[1], alpha),
                    composite(blue, alpha),
                ]),
            );
        }
    }
    Ok(image)
}

fn decode_bytes(
    bytes: Vec<u8>,
    config: &OcrConfig,
    operation: &Operation,
) -> Result<RgbImage, Failure> {
    let probe = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| invalid("无法识别图片格式").with_source(error))?;
    let (width, height) = probe
        .into_dimensions()
        .map_err(|error| invalid("无法读取图片尺寸").with_source(error))?;
    dimensions(width, height, config)?;
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| invalid("无法识别图片格式").with_source(error))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(config.max_pixels * 8);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| invalid("图片解码失败").with_source(error))?
        .to_rgba8();
    let mut rgb = RgbImage::new(width, height);
    for (y, row) in decoded.enumerate_rows() {
        operation.check("image_alpha")?;
        for (x, _, pixel) in row {
            rgb.put_pixel(
                x,
                y,
                Rgb([
                    composite(pixel[0], pixel[3]),
                    composite(pixel[1], pixel[3]),
                    composite(pixel[2], pixel[3]),
                ]),
            );
        }
    }
    Ok(rgb)
}

fn composite(channel: u8, alpha: u8) -> u8 {
    ((u32::from(channel) * u32::from(alpha) + 255 * (255 - u32::from(alpha)) + 127) / 255) as u8
}
fn dimensions(width: u32, height: u32, config: &OcrConfig) -> Result<(), Failure> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > config.max_pixels {
        return Err(Failure::new(
            FailureKind::ResourceLimit,
            "image_dimensions",
            "图片尺寸为零或像素数量超限",
        ));
    }
    Ok(())
}
fn encoded_size(length: usize, config: &OcrConfig) -> Result<(), Failure> {
    if length == 0 || length > config.max_encoded_bytes {
        return Err(Failure::new(
            FailureKind::ResourceLimit,
            "image_bytes",
            "编码图片为空或字节数超限",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::InvalidInput, "image_decode", message)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/image/input.rs"]
mod tests;
