//! 初始缩略图与同一像素来源的差分裁剪，限制累计模型图片开销。
use super::*;
use argusflow_recorder::{Visual, read_attachment};
use base64::Engine;
use image::{DynamicImage, GenericImageView};
use serde_json::{Value, json};
fn decode(directory: &Path, visual: &Visual) -> Result<DynamicImage> {
    let attachment = visual
        .image
        .as_ref()
        .ok_or_else(|| AiError::Invalid("此记录没有图片".into()))?;
    if attachment.bytes > 12 * 1024 * 1024
        || u64::from(attachment.size[0]) * u64::from(attachment.size[1]) > 40_000_000
    {
        return Err(AiError::Budget("图片附件超限".into()));
    }
    let bytes =
        read_attachment(directory, attachment).map_err(|e| AiError::Invalid(e.to_string()))?;
    let dimensions =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|_| AiError::Invalid("PNG 尺寸无效".into()))?;
    if [dimensions.0, dimensions.1] != attachment.size {
        return Err(AiError::Invalid("PNG 实际尺寸与附件声明不符".into()));
    }
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| AiError::Invalid("PNG 附件无法解码".into()))
}
fn content(image: DynamicImage, caption: Value) -> Result<(Vec<Value>, u64)> {
    let image = image.thumbnail(image.width().min(600), image.height().min(300));
    let pixels = u64::from(image.width()) * u64::from(image.height());
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|_| AiError::Invalid("图片编码失败".into()))?;
    Ok((
        vec![
            json!({"type":"text","text":caption.to_string()}),
            json!({"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(encoded.into_inner()))}}),
        ],
        pixels,
    ))
}
impl Evidence {
    pub(crate) fn overview(&self) -> Result<(Vec<Value>, u64)> {
        for record in self.records.values() {
            if let RecordData::Visual(v) = &record.data
                && v.image.is_some()
            {
                return content(
                    decode(&self.directory, v)?,
                    json!({"overview_id":record.id.to_string(),"meaning":"仅开场状态，不代表后续状态"}),
                );
            }
        }
        Ok((Vec::new(), 0))
    }
    pub(crate) fn change(&self, id: &str) -> Result<(Vec<Value>, u64)> {
        let id = id
            .parse::<u64>()
            .map_err(|_| AiError::Invalid("图像 ID 无效".into()))?;
        let after = match self.records.get(&id).map(|r| &r.data) {
            Some(RecordData::Visual(v)) => v,
            _ => return Err(AiError::Invalid("ID 不是视觉证据".into())),
        };
        let (before_id, before) = self
            .records
            .range(..id)
            .rev()
            .find_map(|(id, r)| match &r.data {
                RecordData::Visual(v)
                    if v.version.source == after.version.source
                        && v.version.session == after.version.session
                        && v.version.generation == after.version.generation
                        && v.region == after.region
                        && v.screen_origin == after.screen_origin
                        && v.image.is_some() =>
                {
                    Some((*id, v))
                }
                _ => None,
            })
            .ok_or_else(|| AiError::Invalid("没有同来源的前帧，不能伪造前后差分".into()))?;
        let a = decode(&self.directory, before)?;
        let b = decode(&self.directory, after)?;
        if a.dimensions() != b.dimensions() {
            return Err(AiError::Invalid("前后帧尺寸改变".into()));
        }
        let (w, h) = a.dimensions();
        let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
        let ar = a.to_rgb8();
        let br = b.to_rgb8();
        for y in 0..h {
            for x in 0..w {
                if ar
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(br.get_pixel(x, y).0)
                    .any(|(a, b)| a.abs_diff(b) > 12)
                {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                }
            }
        }
        if right <= left || bottom <= top {
            return Ok((
                vec![json!({"type":"text","text":"没有检测到可见像素变化"})],
                0,
            ));
        }
        left = left.saturating_sub(12);
        top = top.saturating_sub(12);
        right = (right + 12).min(w);
        bottom = (bottom + 12).min(h);
        if u64::from(right - left) * u64::from(bottom - top) > u64::from(w) * u64::from(h) * 3 / 4 {
            return Err(AiError::Invalid(
                "变化覆盖大部分屏幕，拒绝把整屏当局部差分发送；请依据结构证据".into(),
            ));
        }
        let crop = [left, top, right - left, bottom - top];
        let (mut blocks, p1) = content(
            a.crop_imm(crop[0], crop[1], crop[2], crop[3]),
            json!({"id":before_id.to_string(),"phase":"before","crop":crop}),
        )?;
        let (next, p2) = content(
            b.crop_imm(crop[0], crop[1], crop[2], crop[3]),
            json!({"id":id.to_string(),"phase":"after","crop":crop}),
        )?;
        blocks.extend(next);
        Ok((blocks, p1 + p2))
    }
}
