use super::*;
#[test]
fn dropping_caller_cancels_all_copies_without_losing_effect() {
    let operation = Operation::new(OperationOptions::default());
    operation.begin_effect("click").unwrap();
    drop(operation.cancel_on_drop());
    let failure = operation.check("response").unwrap_err();
    assert_eq!(failure.kind(), FailureKind::Cancelled);
    assert_eq!(failure.effect(), Effect::Unconfirmed);
}
#[test]
fn invalid_timeouts_fail_before_dispatch() {
    assert!(OperationOptions::new(Duration::ZERO).is_err());
    assert!(OperationOptions::new(Duration::MAX).is_err());
}
