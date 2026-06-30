use std::{path::PathBuf, process::Command};

use serde_json::Value;

#[test]
fn list_json_discovers_fixture_workspace() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-workspace");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-test-tui"))
        .args([
            "--current-dir",
            fixture.to_str().expect("fixture path"),
            "--list-json",
        ])
        .output()
        .expect("ran cargo-test-tui");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let tests: Vec<Value> = serde_json::from_slice(&output.stdout).expect("json output");
    let names = tests
        .iter()
        .map(|test| {
            (
                test["package"].as_str().expect("package"),
                test["full_name"].as_str().expect("full_name"),
                test["key"]["event_prefix"].as_str().expect("event_prefix"),
            )
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&("alpha", "tests::duplicate_name", "alpha::alpha")));
    assert!(names.contains(&("alpha", "tests::alpha_only", "alpha::alpha")));
    assert!(names.contains(&("beta", "tests::duplicate_name", "beta::beta")));
    assert!(names.contains(&("beta", "tests::beta_only", "beta::beta")));
}
