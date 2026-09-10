//! 对调用方暴露来源明确、资源身份受控的绑定和结果。
use argusflow_browser::{BrowserMatch, Page};
#[cfg(windows)]
use argusflow_capture_contracts::{PixelRect, SourceId};
#[cfg(windows)]
use argusflow_vision::{OcrMatch, SampledOcr, SampledOcrResult};
#[cfg(windows)]
use argusflow_windows::{InputService, UiaMatch, UiaRuntime, WindowIdentity};

/// 查询来源由调用者指定，不自动切换或融合。
#[derive(Clone)]
pub enum QuerySource {
    /// 已附加的浏览器页面。
    Browser(Page),
    /// 已验证窗口中的 UIA 元素。
    #[cfg(windows)]
    Uia {
        /// 复用的 MTA 服务。
        runtime: UiaRuntime,
        /// 受身份验证的目标窗口。
        window: WindowIdentity,
        /// 复用的真实输入服务。
        input: InputService,
    },
    /// 有明确窗口归属的屏幕 OCR 区域。
    #[cfg(windows)]
    Ocr {
        /// 复用的区域采样 OCR 服务。
        sampler: SampledOcr,
        /// 采样来源标识。
        source: SourceId,
        /// 采样来源本地像素区域。
        region: PixelRect,
        /// 输入只允许在此窗口执行。
        window: WindowIdentity,
        /// 复用的真实输入服务。
        input: InputService,
    },
}
/// 定位返回只读快照和平台身份，不把不同来源伪装为同一实体。
#[derive(Clone)]
pub enum LocatedElement {
    /// 带 frame 身份的浏览器节点。
    Browser(BrowserMatch),
    /// 受租约限制的 UIA 元素。
    #[cfg(windows)]
    Uia(UiaMatch),
    /// 识别文字及采样版本。
    #[cfg(windows)]
    Ocr {
        /// 文字与局部几何。
        target: OcrMatch,
        /// 支持输入前内容复验的采样结果。
        sample: SampledOcrResult,
    },
}
