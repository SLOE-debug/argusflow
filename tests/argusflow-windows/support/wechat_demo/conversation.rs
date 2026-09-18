//! 微信会话判断与有限恢复；先观察再决定，恢复时不使用旧坐标。
pub use super::conversation_state::{ASSISTANT, state};
use super::{
    desktop::Desktop,
    layout::Zone,
    observation::Observer,
    recovery::{ConversationState, Decision, decide},
    snapshot::Observation,
    spatial::{self, Direction},
};
use std::{error::Error, num::NonZeroUsize, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// 只允许首次打开和空白恢复各一次；已经打开时零点击。
pub async fn ensure_open(desktop: &Desktop, observer: &mut Observer) -> Result<Observation> {
    let mut snapshot = observer.refresh(desktop).await?;
    let mut clicks = 0;
    loop {
        let current = state(&snapshot)?;
        match decide(current, clicks) {
            Decision::Keep => {
                println!("会话已确认，联系人点击次数={clicks}（已打开时不再点击）");
                return Ok(snapshot);
            }
            Decision::Stop => {
                return Err(format!(
                    "会话恢复结束：状态 {current:?}，已点击 {clicks} 次；未输入或发送消息"
                )
                .into());
            }
            Decision::Open => println!("当前会话 {current:?}，从新快照定位目标联系人"),
            Decision::Recover => println!("点击后会话区域为空白，重新定位并恢复一次"),
        }
        click_contact(desktop, observer, &snapshot).await?;
        clicks += 1;
        // 先等待界面完成切换，避免把短暂空白当作失败并连续点击。
        for delay in [150, 250, 400] {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            snapshot = observer.refresh(desktop).await?;
            if state(&snapshot)? == ConversationState::Ready {
                break;
            }
        }
    }
}
async fn click_contact(
    desktop: &Desktop,
    observer: &Observer,
    snapshot: &Observation,
) -> Result<()> {
    let bounds = snapshot.frame.bounds;
    let header = Zone::Header.region(bounds)?;
    let sidebar = Zone::Sidebar.region(bounds)?;
    let searches = snapshot.text(header, "搜索", true)?;
    let anchor = spatial::top_left(&searches).ok_or("搜索锚点消失")?;
    let contacts = snapshot.text(sidebar, ASSISTANT, false)?;
    let contact = spatial::nearest(
        &contacts,
        anchor,
        Direction::Below,
        NonZeroUsize::MIN,
        sidebar.width(),
    )
    .ok_or("目标联系人不在可见列表中")?;
    let region = snapshot.blocks[contact.index]
        .rect
        .expanded(4, bounds.local())
        .ok_or("联系人区域失效")?;
    observer.confirm(desktop, snapshot, region).await?;
    println!("点击新识别的联系人坐标 {:?}", contact.point);
    desktop.click(contact.point, bounds).await
}
