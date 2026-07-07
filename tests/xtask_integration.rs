use std::{path::PathBuf, process::Command};

use serde_json::Value;

#[test]
fn list_xtasks_json_discovers_nextdeck_xtask_endpoint() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_nextdeck"))
        .args([
            "--current-dir",
            root.to_str().expect("workspace path"),
            "--list-xtasks-json",
        ])
        .output()
        .expect("ran nextdeck");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(manifest["schema_version"], 1);

    let commands = manifest["commands"].as_array().expect("commands");
    assert!(
        commands
            .iter()
            .any(|command| command["name"] == "tui-check")
    );
    assert!(
        commands
            .iter()
            .any(|command| command["name"] == "lib-check")
    );
    assert!(
        commands
            .iter()
            .any(|command| command["name"] == "lib-publish-local")
    );
    for removed in ["check", "package", "install-path", "install-package"] {
        assert!(
            !commands.iter().any(|command| command["name"] == removed),
            "{removed} should not be exposed"
        );
    }
    let release = commands
        .iter()
        .find(|command| command["name"] == "tui-release")
        .expect("tui-release command");
    let release_args = release["args"].as_array().expect("release args");
    assert!(
        release_args
            .iter()
            .any(|arg| { arg["long"] == "skip-sign" && arg["value"]["type"] == "bool" })
    );
}
