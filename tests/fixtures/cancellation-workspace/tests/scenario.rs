use std::{fs, thread, time::Duration};

#[test]
fn scenario_finishes_successfully_with_error_logs() {
    fs::create_dir_all("target").expect("create fixture target directory");
    fs::write("target/cancellation-scenario-started", "started")
        .expect("write scenario start marker");
    println!("CANCELLATION_SCENARIO_STARTED: continuing during fail-fast wait");
    thread::sleep(Duration::from_millis(2200));
    eprintln!("[ERR] CANCELLATION_SCENARIO_CLEANUP: non-fatal cleanup error");
    println!("CANCELLATION_SCENARIO_RETURNING_OK: scenario completed");
}
