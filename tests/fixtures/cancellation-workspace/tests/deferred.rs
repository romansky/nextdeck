#[test]
fn deferred_test_must_not_start_after_fail_fast() {
    panic!("CANCELLATION_DEFERRED_STARTED: fail-fast should have skipped this test");
}
