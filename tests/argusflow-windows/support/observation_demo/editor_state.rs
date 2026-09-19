//! 当前微信空输入框的灰色语音提示；真实亮色草稿不能当作占位提示。
use super::wechat_support::{editor, snapshot::Observation};
use argusflow_capture_contracts::PixelRect;

pub fn is_empty(snapshot: &Observation, region: PixelRect) -> bool {
    editor::is_empty(&without_placeholder(snapshot), region)
}

pub fn without_placeholder(snapshot: &Observation) -> Observation {
    let mut content = snapshot.clone();
    std::sync::Arc::make_mut(&mut content.blocks).retain(|block| {
        if block.text.split_whitespace().collect::<String>() != "按住鼠标语音输入文字" {
            return true;
        }
        let width = snapshot.frame.bounds.width() as usize;
        (block.rect.y()..block.rect.bottom()).any(|y| {
            (block.rect.x()..block.rect.right()).any(|x| {
                let offset = (y as usize * width + x as usize) * 3;
                snapshot.frame.pixels[offset..offset + 3]
                    .iter()
                    .all(|v| *v > 180)
            })
        })
    });
    content
}
