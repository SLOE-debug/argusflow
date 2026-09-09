//! PP-OCRv6 CTC：blank=0，相邻重复合并，空格追加到官方字典末尾。
use crate::OcrError as Failure;
use argusflow_core::FailureKind;

pub(crate) fn ctc(
    values: &[f32],
    steps: usize,
    classes: usize,
    dictionary: &[String],
) -> Result<(String, f32), Failure> {
    if classes != dictionary.len()
        || classes < 2
        || steps.checked_mul(classes) != Some(values.len())
    {
        return Err(invalid("识别输出维度与字符字典不匹配"));
    }
    let mut text = String::new();
    let mut previous = 0;
    let mut score_sum = 0f32;
    let mut count = 0;
    for step in values.chunks_exact(classes) {
        if step
            .iter()
            .any(|score| !score.is_finite() || *score < 0.0 || *score > 1.0)
        {
            return Err(invalid("识别模型输出非概率或非有限值"));
        }
        let (index, score) = step
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .ok_or_else(|| invalid("识别输出为空"))?;
        if index != 0 && index != previous {
            text.push_str(&dictionary[index]);
            score_sum += *score;
            count += 1;
        }
        previous = index;
    }
    Ok((
        text,
        if count == 0 {
            0.0
        } else {
            score_sum / count as f32
        },
    ))
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::Protocol, "ctc_decode", message)
}

#[cfg(test)]
#[path = "../../../../../tests/argusflow-vision/unit/ocr/recognition/decode.rs"]
mod tests;
