#![cfg(windows)]
//! 专属测试控件上的只读观察验收；不向日常应用注入输入。
#[path = "../support/window.rs"]
#[allow(dead_code)]
mod support;
use argusflow_windows::*;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_SETPASSWORDCHAR, EM_SETSEL};
use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::HSTRING,
};

fn selected(value: &UiaObservation) -> &UiaTextSnapshot {
    let UiaTextObservation::Available(text) = &value.text else {
        panic!("TextPattern 未提供快照: {:?}", value.text);
    };
    text
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "创建专属原生测试窗口，读取实际 TextPattern 选区"]
async fn native_selection_is_read_only_and_distinguishes_duplicates() {
    let _dpi = support::DpiContext::physical();
    let fixture = support::Fixture::create();
    let runtime = UiaRuntime::start(UiaConfig::default(), OperationOptions::default())
        .await
        .unwrap();
    let handle = fixture.child(102);
    let mut rect = RECT::default();
    unsafe {
        SetWindowTextW(handle, &HSTRING::from("same same 😀tail")).unwrap();
        SendMessageW(handle, EM_SETSEL, Some(WPARAM(5)), Some(LPARAM(9)));
        GetWindowRect(handle, &mut rect).unwrap();
    }
    let point = Some([rect.left + 10, rect.top + 10]);
    let first = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert_eq!(selected(&first).selections[0].text, "same");
    assert_eq!(selected(&first).selections[0].start_utf16, Some(5));
    assert_eq!(selected(&first).selections[0].end_utf16, Some(9));
    let second = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert_eq!(
        second.text_change_from(Some(&first)),
        UiaTextChange::Compared {
            document_changed: false,
            selection_changed: false
        }
    );
    let mut start = 0u32;
    let mut end = 0u32;
    unsafe {
        SendMessageW(
            handle,
            EM_GETSEL,
            Some(WPARAM((&mut start as *mut u32) as usize)),
            Some(LPARAM((&mut end as *mut u32) as isize)),
        );
    }
    assert_eq!((start, end), (5, 9), "观察不能收起或改变原选区");
    unsafe {
        SendMessageW(handle, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(4)));
    }
    let third = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert_eq!(selected(&third).selections[0].text, "same");
    assert_eq!(
        third.text_change_from(Some(&second)),
        UiaTextChange::Compared {
            document_changed: false,
            selection_changed: true
        }
    );
    unsafe {
        SendMessageW(handle, EM_SETSEL, Some(WPARAM(10)), Some(LPARAM(12)));
    }
    let unicode = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert_eq!(selected(&unicode).selections[0].text, "😀");
    assert_eq!(selected(&unicode).selections[0].end_utf16, Some(12));
    unsafe {
        SendMessageW(handle, EM_SETSEL, Some(WPARAM(12)), Some(LPARAM(12)));
    }
    let caret = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert!(selected(&caret).selections[0].collapsed);
    assert!(selected(&caret).selections[0].text.is_empty());
    unsafe {
        SetWindowTextW(handle, &HSTRING::from("a".repeat(9000))).unwrap();
        SendMessageW(handle, EM_SETSEL, Some(WPARAM(8500)), Some(LPARAM(8504)));
    }
    let limited = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert!(selected(&limited).document_truncated);
    assert_eq!(selected(&limited).selections[0].start_utf16, Some(8500));
    assert_eq!(selected(&limited).selections[0].text, "aaaa");
    assert_eq!(
        limited.text_change_from(Some(&caret)),
        UiaTextChange::Unresolved
    );
    unsafe {
        SendMessageW(handle, EM_SETPASSWORDCHAR, Some(WPARAM(42)), None);
    }
    let sensitive = runtime
        .observe_target(point, OperationOptions::default())
        .await
        .unwrap();
    assert!(sensitive.target.password);
    assert_eq!(sensitive.target.value, None);
    assert_eq!(sensitive.text, UiaTextObservation::Sensitive);
    runtime.shutdown(OperationOptions::default()).await.unwrap();
}

#[tokio::test]
#[ignore = "只读系统剪贴板，不修改或输出实际内容"]
async fn native_clipboard_snapshot_and_shutdown() {
    let reader = ClipboardReader::start().unwrap();
    let first = reader
        .observe(None, OperationOptions::default())
        .await
        .unwrap();
    let second = reader
        .observe(Some(first.sequence), OperationOptions::default())
        .await
        .unwrap();
    if first.sequence == second.sequence {
        assert_eq!(
            second.content,
            argusflow_input_contracts::ClipboardContent::Unchanged
        );
    }
    reader.shutdown(OperationOptions::default()).await.unwrap();
    assert!(
        reader
            .observe(None, OperationOptions::default())
            .await
            .is_err()
    );
}
