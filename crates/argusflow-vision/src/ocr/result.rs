//! OCR 输出只表达原图文字与几何，不含屏幕或工作流语义。
use argusflow_core::ImagePoint;

/// 原图中的一个文本区域，四角按左上、右上、右下、左下排列。
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub(crate) text: String,
    pub(crate) confidence: f32,
    pub(crate) polygon: [ImagePoint; 4],
}
impl TextBlock {
    /// CTC 解码的完整文字。
    pub fn text(&self) -> &str {
        &self.text
    }
    /// 保留字符的平均置信度，范围为 0..=1。
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
    /// 原图像素坐标四边形。
    pub fn polygon(&self) -> &[ImagePoint; 4] {
        &self.polygon
    }
}

/// 一张图片的识别结果；无文字是成功的空列表。
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub(crate) blocks: Vec<TextBlock>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
impl OcrResult {
    /// 按从上到下、同行从左到右排列的文本块。
    pub fn blocks(&self) -> &[TextBlock] {
        &self.blocks
    }
    /// 每个文本块占一行的全文。
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// 原图宽度。
    pub fn width(&self) -> u32 {
        self.width
    }
    /// 原图高度。
    pub fn height(&self) -> u32 {
        self.height
    }
}
