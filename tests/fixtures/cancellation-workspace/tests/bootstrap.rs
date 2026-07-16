use std::{path::Path, thread, time::Duration};

#[test]
fn bootstrap_fails_after_scenario_starts() {
    let scenario_marker = Path::new("target/cancellation-scenario-started");
    for _ in 0..100 {
        if scenario_marker.exists() {
            thread::sleep(Duration::from_millis(50));
            panic!("CANCELLATION_BOOTSTRAP_FAILURE: expected fixture failure");
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("CANCELLATION_BOOTSTRAP_TIMEOUT: scenario fixture did not start");
}
