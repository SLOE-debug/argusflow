use super::*;
use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};

pub(super) async fn exercise(
    fixture: &support::Fixture,
    window: &WindowIdentity,
    input: &InputService,
) -> Result<(), Box<dyn std::error::Error>> {
    for nested in [false, true] {
        unsafe {
            PostMessageW(
                Some(fixture.hwnd()),
                support::menu::OPEN,
                WPARAM(0),
                LPARAM(0),
            )?;
        }
        wait(|| item(false, 0).is_some()).await?;
        let point = item(false, 0).unwrap();
        let surfaces = window.physical_surfaces()?;
        assert!(surfaces.len() > 1, "active popup is part of OCR scope");
        let bounds = window.physical_bounds()?;
        println!("menu point={point:?}; main={bounds:?}; surfaces={surfaces:?}");
        assert!(
            point.x >= bounds.x() + bounds.width() as i32,
            "test menu must lie outside main window"
        );
        let target = if nested {
            input
                .perform(
                    window.clone(),
                    InputAction::Click {
                        point: item(false, 1).unwrap(),
                        button: MouseButton::Left,
                        count: ClickCount::Single,
                    },
                    options(),
                )
                .await?;
            wait(|| item(true, 0).is_some()).await?;
            item(true, 0).unwrap()
        } else {
            point
        };
        input
            .perform(
                window.clone(),
                InputAction::Click {
                    point: target,
                    button: MouseButton::Left,
                    count: ClickCount::Single,
                },
                options(),
            )
            .await?;
        wait(|| support::menu::SELECTED.load(Ordering::SeqCst) == if nested { 902 } else { 901 })
            .await?;
        wait(|| window.physical_surfaces().is_ok_and(|s| s.len() == 1)).await?;
    }
    println!(
        "PASS physical menu and nested-menu clicks outside main bounds; OCR popup scope removal"
    );
    Ok(())
}
fn item(nested: bool, index: u32) -> Option<ScreenPoint> {
    let handle = if nested {
        support::menu::SUBMENU.load(Ordering::SeqCst)
    } else {
        support::menu::MENU.load(Ordering::SeqCst)
    };
    if handle == 0 {
        return None;
    }
    let mut rect = RECT::default();
    if unsafe { GetMenuItemRect(None, HMENU(handle as *mut _), index, &mut rect) }.is_err()
        || rect.right <= rect.left
        || rect.bottom <= rect.top
    {
        return None;
    }
    Some(ScreenPoint {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    })
}
