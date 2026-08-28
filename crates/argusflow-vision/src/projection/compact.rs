//! CompactText 的确定性行和列投影。

use crate::{
    projection::ProjectionOptions,
    scene::{VisualRegionKind, VisualScene},
};

/// 将 VisualScene 投影成适合日志、规则和模型 context 的文本。
pub fn compact_text(scene: &VisualScene, options: &ProjectionOptions) -> String {
    if scene.nodes.is_empty() {
        return String::new();
    }
    let mut output = String::new();
    if options.region_markers {
        for region in &scene.regions {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push('[');
            output.push_str(region_label(region.kind));
            output.push(']');
        }
    }
    for line in &scene.lines {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&line.text);
    }
    output
}

/// 返回开发者可读且稳定的 region 名称。
fn region_label(kind: VisualRegionKind) -> &'static str {
    match kind {
        VisualRegionKind::Navigation => "Navigation",
        VisualRegionKind::Sidebar => "Sidebar",
        VisualRegionKind::List => "List",
        VisualRegionKind::Header => "Header",
        VisualRegionKind::Content => "Content",
        VisualRegionKind::ChatHistory => "ChatHistory",
        VisualRegionKind::Editor => "Editor",
        VisualRegionKind::Popup => "Popup",
        VisualRegionKind::Dialog => "Dialog",
        VisualRegionKind::Unknown => "Unknown",
    }
}
