//! 将最近一次失败的 OCR 输入保存为无需额外图片依赖即可查看的 BMP。

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde_json::json;

use crate::{error::SceneExecutionPhase, frame::PixelFormat, image::PixelImage, ocr::OcrRequest};

/// 开发环境显式指定视觉失败现场目录使用的环境变量。
const DIAGNOSTICS_DIRECTORY_ENV: &str = "ARGUSFLOW_VISION_DIAGNOSTICS_DIR";
/// 最近一次超时的结构化元数据文件名。
const TIMEOUT_METADATA_FILE: &str = "last-scene-timeout.json";
/// 最近一次失败时已经生成的 OCR 输入图片文件名。
const OCR_INPUT_IMAGE_FILE: &str = "last-ocr-input.bmp";

/// 持久化最近一次场景超时，并返回可直接进入运行日志的诊断摘要。
pub(crate) fn persist_scene_timeout(
    phase: SceneExecutionPhase,
    timeout_ms: u64,
    request: Option<&OcrRequest>,
) -> String {
    let Some(directory) = configured_directory() else {
        return input_state_summary(request, "failure diagnostics are not enabled");
    };

    match persist_scene_timeout_to(&directory, phase, timeout_ms, request) {
        Ok(image_path) => match image_path {
            Some(path) => format!(
                "OCR input was created; image: {}; metadata: {}",
                path.display(),
                directory.join(TIMEOUT_METADATA_FILE).display()
            ),
            None => format!(
                "OCR input was not created before the timeout; metadata: {}",
                directory.join(TIMEOUT_METADATA_FILE).display()
            ),
        },
        Err(error) => input_state_summary(
            request,
            &format!(
                "failed to save diagnostics in {}: {error}",
                directory.display()
            ),
        ),
    }
}

/// 读取进程级诊断目录；未显式配置时绝不落盘窗口内容。
fn configured_directory() -> Option<PathBuf> {
    std::env::var_os(DIAGNOSTICS_DIRECTORY_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 写入超时元数据，并在已经形成 OCR 请求时额外写入其像素图片。
fn persist_scene_timeout_to(
    directory: &Path,
    phase: SceneExecutionPhase,
    timeout_ms: u64,
    request: Option<&OcrRequest>,
) -> Result<Option<PathBuf>, String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let image_path = directory.join(OCR_INPUT_IMAGE_FILE);
    let persisted_image_path = if let Some(request) = request {
        let bitmap = encode_bgra_as_bmp(&request.image)?;
        fs::write(&image_path, bitmap).map_err(|error| error.to_string())?;
        Some(image_path)
    } else {
        remove_stale_file(&image_path)?;
        None
    };

    let input_metadata = request.map(|request| {
        json!({
            "request_id": request.request_id.as_uuid(),
            "window": request.window,
            "frame_id": request.frame_id.get(),
            "topology_generation": request.topology_generation.get(),
            "model": request.profile.model,
            "options": &request.profile.options,
            "roi": request.roi,
            "width": request.image.width,
            "height": request.image.height,
            "stride_bytes": request.image.stride_bytes,
            "pixel_format": request.image.format,
            "deadline_ms": duration_ms(request.deadline),
        })
    });
    let metadata = json!({
        "schema_version": 1,
        "timeout_ms": timeout_ms,
        "phase": phase.as_str(),
        "ocr_input_created": request.is_some(),
        "ocr_input_image": persisted_image_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        "ocr_input": input_metadata,
    });
    let serialized = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
    fs::write(directory.join(TIMEOUT_METADATA_FILE), serialized)
        .map_err(|error| error.to_string())?;
    Ok(persisted_image_path)
}

/// 删除上一轮留下的图片，确保目录始终只描述最近一次失败。
fn remove_stale_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// 将持续时间压缩为日志使用的无符号毫秒数。
fn duration_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

/// 生成不依赖目录配置的输入形成状态摘要。
fn input_state_summary(request: Option<&OcrRequest>, detail: &str) -> String {
    if request.is_some() {
        format!("OCR input was created, but {detail}")
    } else {
        format!("OCR input was not created before the timeout; {detail}")
    }
}

/// 把 OCR 使用的 BGRA 图片原样编码为常见查看器可直接打开的 32 位 top-down BMP。
pub fn encode_bgra_as_bmp(image: &PixelImage) -> Result<Vec<u8>, String> {
    if image.format != PixelFormat::Bgra8Unorm {
        return Err(format!(
            "unsupported diagnostic pixel format: {:?}",
            image.format
        ));
    }
    let width = i32::try_from(image.width).map_err(|_| "image width exceeds BMP limits")?;
    let height = i32::try_from(image.height).map_err(|_| "image height exceeds BMP limits")?;
    let source_row_bytes = image.width as usize * image.format.bytes_per_pixel();
    let pixel_bytes = source_row_bytes
        .checked_mul(image.height as usize)
        .ok_or("BMP pixel byte length overflow")?;
    let file_bytes = 54_usize
        .checked_add(pixel_bytes)
        .ok_or("BMP file byte length overflow")?;
    let file_size = u32::try_from(file_bytes).map_err(|_| "BMP file exceeds 4 GiB")?;
    let pixel_size = u32::try_from(pixel_bytes).map_err(|_| "BMP pixels exceed 4 GiB")?;

    let mut output = Vec::with_capacity(file_bytes);
    output.extend_from_slice(b"BM");
    output.extend_from_slice(&file_size.to_le_bytes());
    output.extend_from_slice(&[0_u8; 4]);
    output.extend_from_slice(&54_u32.to_le_bytes());
    output.extend_from_slice(&40_u32.to_le_bytes());
    output.extend_from_slice(&width.to_le_bytes());
    // 负高度使 BMP 按 OCR 输入相同的从上到下行序保存，避免额外翻转。
    output.extend_from_slice(&(-height).to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&32_u16.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&pixel_size.to_le_bytes());
    output.extend_from_slice(&[0_u8; 16]);

    for row in 0..image.height as usize {
        let row_start = row * image.stride_bytes;
        let source_row = image
            .pixels()
            .get(row_start..row_start + source_row_bytes)
            .ok_or("pixel image row is shorter than its declared stride")?;
        output.extend_from_slice(source_row);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_encoder_preserves_top_down_bgra_pixels() {
        let image = PixelImage::new(
            2,
            1,
            8,
            PixelFormat::Bgra8Unorm,
            vec![1, 2, 3, 0, 4, 5, 6, 255],
        )
        .expect("fixture is a valid BGRA image");

        let bitmap = encode_bgra_as_bmp(&image).expect("diagnostic BMP should encode");

        assert_eq!(&bitmap[..2], b"BM");
        assert_eq!(u32::from_le_bytes(bitmap[2..6].try_into().unwrap()), 62);
        assert_eq!(i32::from_le_bytes(bitmap[22..26].try_into().unwrap()), -1);
        assert_eq!(&bitmap[54..62], &[1, 2, 3, 0, 4, 5, 6, 255]);
    }
}
