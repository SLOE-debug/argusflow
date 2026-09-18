//! 显式应用目标与 demo 执行模式。
use super::application::ApplicationTarget;
use std::{error::Error, path::PathBuf};

/// 观察不导航；打开不发送；发送严格执行一次。
pub enum Mode {
    /// 输出全窗口 OCR 并连续刷新两次，检查差分统计。
    Observe,
    /// 只确保文件传输助手已打开。
    Open,
    /// 发送一条短消息。
    Send(String),
}
/// 装配所需依赖与目标，不把微信路径写死在平台实现。
pub struct Config {
    /// Paddle 模型及 ORT 依赖目录。
    pub dependencies: PathBuf,
    /// 可执行文件与唯一窗口标题。
    pub target: ApplicationTarget,
    /// 此次运行的明确副作用范围。
    pub mode: Mode,
}
impl Config {
    /// 拒绝额外参数、换行和空消息。
    pub fn parse() -> Result<Self, Box<dyn Error>> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() < 4 {
            return Err(usage().into());
        }
        let mode = match &args[3..] {
            [mode] if mode == "observe" => Mode::Observe,
            [mode] if mode == "open" => Mode::Open,
            [mode, message]
                if mode == "send"
                    && !message.trim().is_empty()
                    && message.chars().count() <= 80
                    && !message.chars().any(char::is_control) =>
            {
                Mode::Send(message.clone())
            }
            _ => return Err(usage().into()),
        };
        Ok(Self {
            dependencies: (&args[0]).into(),
            target: ApplicationTarget {
                executable: (&args[1]).into(),
                title: args[2].clone(),
            },
            mode,
        })
    }
}
fn usage() -> &'static str {
    "用法: wechat_ocr_demo <deps目录> <EXE绝对路径> <窗口标题> observe|open|send [1-80字单行消息]"
}
