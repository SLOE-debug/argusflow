use super::*;
use argusflow_core::{Effect, FailureKind};

#[tokio::test]
async fn responses_are_correlated_out_of_order() {
    let mut mock = Mock::new(2).await;
    let first = mock.connection.clone();
    let second = mock.connection.clone();
    let a = tokio::spawn(async move {
        first
            .command(
                None,
                "Test.a",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    let request_a = mock.next().await;
    let b = tokio::spawn(async move {
        second
            .command(
                None,
                "Test.b",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    let request_b = mock.next().await;
    mock.respond(&request_b, json!({"value":2})).await;
    mock.respond(&request_a, json!({"value":1})).await;
    assert_eq!(a.await.unwrap().unwrap()["value"], 1);
    assert_eq!(b.await.unwrap().unwrap()["value"], 2);
    mock.connection
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn timeout_discards_late_response_and_never_replays_effect() {
    let mut mock = Mock::new(1).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        let op = Operation::new(options(80));
        let _guard = op.cancel_on_drop();
        connection
            .command(
                None,
                "Input.insertText",
                json!({"text":"private"}),
                true,
                &op,
            )
            .await
    });
    let request = mock.next().await;
    let failure = task.await.unwrap().unwrap_err();
    assert_eq!(failure.kind(), FailureKind::Timeout);
    assert_eq!(failure.effect(), Effect::Unconfirmed);
    mock.respond(&request, json!({})).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        connection
            .command(
                None,
                "Test.next",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    let request = mock.next().await;
    assert_eq!(request["method"], "Test.next");
    mock.respond(&request, json!({})).await;
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn full_budget_fails_fast_and_dropped_future_releases_slot() {
    let mut mock = Mock::new(1).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        let op = Operation::new(options(1000));
        let _guard = op.cancel_on_drop();
        connection
            .command(None, "Test.block", json!({}), false, &op)
            .await
    });
    mock.next().await;
    let failure = mock
        .connection
        .command(
            None,
            "Test.overflow",
            json!({}),
            false,
            &Operation::new(options(1000)),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.kind(), FailureKind::Busy);
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        connection
            .command(
                None,
                "Test.recovered",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    let request = mock.next().await;
    assert_eq!(request["method"], "Test.recovered");
    mock.respond(&request, json!({})).await;
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn malformed_frame_disconnects_and_clears_pending() {
    let mut mock = Mock::new(2).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        connection
            .command(
                None,
                "Test.wait",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    mock.next().await;
    mock.socket
        .send(Message::Text("{broken".into()))
        .await
        .unwrap();
    assert_eq!(
        task.await.unwrap().unwrap_err().kind(),
        FailureKind::Protocol
    );
    mock.connection
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        mock.connection.state(),
        crate::ConnectionState::Disconnected
    );
}

#[tokio::test]
async fn simultaneous_shutdown_wakes_pending_without_deadlock() {
    let mut mock = Mock::new(2).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        connection
            .command(
                None,
                "Test.wait",
                json!({}),
                false,
                &Operation::new(options(1000)),
            )
            .await
    });
    mock.next().await;
    let (a, b) = tokio::join!(
        mock.connection.shutdown(Duration::from_secs(1)),
        mock.connection.shutdown(Duration::from_secs(1))
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(task.await.unwrap().unwrap_err().kind(), FailureKind::Closed);
}

#[tokio::test]
async fn peer_disconnect_after_effect_reports_unconfirmed() {
    let mut mock = Mock::new(1).await;
    let connection = mock.connection.clone();
    let task = tokio::spawn(async move {
        connection
            .command(
                None,
                "Input.dispatchMouseEvent",
                json!({}),
                true,
                &Operation::new(options(1000)),
            )
            .await
    });
    mock.next().await;
    mock.socket.close(None).await.unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.effect(), Effect::Unconfirmed);
    assert_eq!(error.kind(), FailureKind::Unavailable);
}
