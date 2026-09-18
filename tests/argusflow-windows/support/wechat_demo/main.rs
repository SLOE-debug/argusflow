//! 最小微信 OCR 工作流；平台输入、视觉观察和空间查询独立组织。
//!
//! 运行：cargo run -p argusflow-windows --release --example wechat_ocr_demo -- .deps <EXE绝对路径> <窗口标题> <模式>
//! 模式：observe 输出整窗文字并刷新差分；open 只打开会话；send "测试消息" 单次发送。
//!
//! 前置条件：微信已登录、文件传输助手在可见列表中，
//! 使用默认双栏比例布局与 Enter 发送。短消息必须能被 OCR 完整识别为一个文本块。
//! 只演示已存在会话，不修改 AQL 语法；空间关系暂存在独立 spatial 模块。
//! 发送检测使用默认绿色气泡及连续短位移假设；需要独占交互、停留在聊天底部。
//! 采样缺口、周期混叠和未知主题返回 Unconfirmed；LocalBubbleObserved 不代表网络送达。
//! 退出码：0 完成观察/本地气泡确认，3 发送结果未确认，1 执行错误；所有分支均不重发。
mod application;
mod bubbles;
mod capture;
mod config;
mod conversation;
mod conversation_state;
mod desktop;
mod editor;
mod frame;
mod incremental;
mod layout;
mod message_tracking;
mod observation;
mod pixel_motion;
mod recovery;
mod scroll_motion;
mod send_trace;
mod send_verification;
mod sending;
mod snapshot;
mod spatial;
mod text;
mod workflow;

#[tokio::main]
async fn main() -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let config = config::Config::parse()?;
    Ok(match workflow::run(config).await? {
        Some(send_verification::SendOutcome::Unconfirmed(_)) => std::process::ExitCode::from(3),
        Some(send_verification::SendOutcome::LocalBubbleObserved) | None => {
            std::process::ExitCode::SUCCESS
        }
    })
}
