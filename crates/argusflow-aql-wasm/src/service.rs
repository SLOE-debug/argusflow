//! 中文编辑文本到唯一英文语言服务的转换及范围回写。
use crate::{localize, translate};
use argusflow_aql::{DiagnosticCode, EditorRange, analyze};
use serde::Serialize;

/// 可直接绘制在中文编辑器里的诊断。
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDiagnostic {
    /// 稳定分类。
    pub code: DiagnosticCode,
    /// 中文消息。
    pub message: String,
    /// 中文文本中的 UTF-16 范围。
    pub range: EditorRange,
}
/// 当前草稿的完整分析结果。
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedDocument {
    /// 仅当前草稿合法时提供规范英文源码。
    pub english: Option<String>,
    /// 中文格式化文本，保留注释和字面量。
    pub formatted: Option<String>,
    /// 当前文本诊断。
    pub diagnostics: Vec<LocalizedDiagnostic>,
}
/// 同步分析当前草稿，决不缓存旧的有效文本。
pub fn inspect(source: &str) -> LocalizedDocument {
    let translated = translate(source);
    let analysis = analyze(translated.source());
    let diagnostics = analysis
        .diagnostics
        .into_iter()
        .map(|diagnostic| LocalizedDiagnostic {
            code: diagnostic.code,
            message: diagnostic.message,
            range: translated.to_original(diagnostic.span).editor_range(source),
        })
        .collect::<Vec<_>>();
    LocalizedDocument {
        english: if diagnostics.is_empty() {
            Some(translated.source().into())
        } else {
            None
        },
        formatted: analysis
            .formatted
            .map(|source| localize(&source).source().to_owned()),
        diagnostics,
    }
}
