use super::*;
use argusflow_input_contracts::ClipboardContent;
use std::time::Duration;

#[tokio::test]
async fn timed_out_source_is_not_replaced_and_shutdown_is_truthful() {
    let (release, waiting) = std::sync::mpsc::channel();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let reader = ClipboardReader::start_with(move |_, _| {
        if let Some(entered) = entered.take() {
            let _ = entered.send(());
        }
        waiting.recv().unwrap();
        Ok(ClipboardObservation {
            previous_sequence: None,
            sequence: 1,
            content: ClipboardContent::NoText,
        })
    })
    .unwrap();
    let clone = reader.clone();
    let call = tokio::spawn(async move {
        clone
            .observe(
                None,
                OperationOptions::new(Duration::from_millis(100)).unwrap(),
            )
            .await
    });
    ready.await.unwrap();
    assert_eq!(
        call.await.unwrap().unwrap_err().kind(),
        FailureKind::Timeout
    );
    assert_eq!(
        reader
            .observe(None, OperationOptions::default())
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Busy
    );
    assert_eq!(
        reader
            .shutdown(OperationOptions::new(Duration::from_millis(20)).unwrap())
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Timeout
    );
    release.send(()).unwrap();
    reader.shutdown(OperationOptions::default()).await.unwrap();
}

#[tokio::test]
async fn same_text_with_a_new_sequence_remains_a_new_observation() {
    let mut sequence = 1;
    let reader = ClipboardReader::start_with(move |previous, _| {
        sequence += 1;
        Ok(ClipboardObservation {
            previous_sequence: previous,
            sequence,
            content: ClipboardContent::Text {
                text: "重复".into(),
                truncated: false,
            },
        })
    })
    .unwrap();
    let first = reader
        .observe(None, OperationOptions::default())
        .await
        .unwrap();
    let second = reader
        .observe(Some(first.sequence), OperationOptions::default())
        .await
        .unwrap();
    assert_ne!(first.sequence, second.sequence);
    assert_eq!(first.content, second.content);
    reader.shutdown(OperationOptions::default()).await.unwrap();
}
