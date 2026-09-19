//! 单次 Enter 的新生气泡验证；相同正文、历史气泡或不确定运动不触发重发。
use super::{
    Result,
    wechat_support::{
        capture, desktop::Desktop, frame::Frame, layout::Zone, observation::Observer, send_trace,
        snapshot::Observation,
    },
};
pub async fn send_once(
    desktop: &Desktop,
    observer: &mut Observer,
    text: &str,
) -> Result<Observation> {
    let bounds = desktop.bounds()?;
    let baseline = Frame::from_bgrx(bounds, capture::capture(desktop, bounds, bounds).await?)?;
    let region = Zone::Messages.region(baseline.bounds)?;
    let trace = send_trace::send_once(desktop, baseline, region).await?;
    let candidate = trace
        .candidate
        .map_err(|e| format!("发送结果未确认：{e:?}；不自动重发"))?;
    let after = observer.recognize_frame(desktop, trace.frame).await?;
    let outcome = super::wechat_support::send_verification::verify(
        &super::editor_state::without_placeholder(&after),
        Ok(candidate),
        text,
    )?;
    if outcome != super::wechat_support::send_verification::SendOutcome::LocalBubbleObserved {
        return Err(format!("新生气泡正文未确认：{outcome:?}；不自动重发").into());
    }
    println!("本次新生气泡轨迹与正文已确认；服务器送达状态未知");
    Ok(after)
}
