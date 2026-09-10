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

#[test]
fn child_cancellation_is_one_way_and_effect_is_attempt_local() {
    let root = Operation::new(OperationOptions::default());
    let child = root.child(OperationOptions::default());
    let sibling = root.child(OperationOptions::default());
    child.begin_effect("action").unwrap();
    child.cancel();
    assert!(!root.is_cancelled());
    assert!(!sibling.is_cancelled());
    assert_eq!(root.effect(), Effect::None);
    assert_eq!(sibling.effect(), Effect::None);
    root.cancel();
    assert!(sibling.is_cancelled());
}

#[test]
fn child_deadline_cannot_extend_parent() {
    let root = Operation::new(OperationOptions::new(Duration::from_millis(50)).unwrap());
    let child = root.child(OperationOptions::new(Duration::from_secs(5)).unwrap());
    let short = root.child(OperationOptions::new(Duration::from_millis(1)).unwrap());
    assert_eq!(child.deadline(), root.deadline());
    assert!(short.deadline() < root.deadline());
}
