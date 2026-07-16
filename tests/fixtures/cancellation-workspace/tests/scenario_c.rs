use std::{fs, thread, time::Duration};

#[test]
fn scenario_c_finishes_successfully_during_cancellation() {
    fs::create_dir_all("target").expect("create fixture target directory");
    fs::write("target/cancellation-scenario-started", "started")
        .expect("write scenario start marker");
    println!("CANCELLATION_SCENARIO_C_STARTED: continuing during fail-fast wait");
    thread::sleep(Duration::from_millis(2300));
    println!("CANCELLATION_SCENARIO_C_RETURNING_OK: scenario completed");
}
