//! 已取得输入排他权后的共享鼠标动作。
use argusflow_core::{ClickCount, Failure, MouseButton, ScreenPoint};
use argusflow_windows::{InputAction, InputSequence, WindowIdentity};
pub(super) async fn click(
    input: &mut InputSequence<'_>,
    window: &WindowIdentity,
    point: ScreenPoint,
) -> Result<(), Failure> {
    input
        .perform(
            window.clone(),
            InputAction::Click {
                point,
                button: MouseButton::Left,
                count: ClickCount::Single,
            },
        )
        .await
        .map_err(Into::into)
}
