use std::{
    env, fs,
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::xtask::{
    SCHEMA_VERSION, XtaskArgSpec, XtaskArgValue, XtaskCommandSpec, XtaskManifest, XtaskValueSpec,
};

#[tokio::test]
async fn resolves_cargo_workspace_root_from_nested_directory() {
    let root = temp_dir("workspace-root");
    let member = root.join("member");
    fs::create_dir_all(member.join("src")).expect("create workspace member");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n",
    )
    .expect("write workspace manifest");
    fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write member manifest");
    fs::write(member.join("src/lib.rs"), "").expect("write member target");

    let resolved = resolve_workspace_root(Some(member))
        .await
        .expect("workspace root");

    assert_eq!(resolved, root.canonicalize().expect("canonical root"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn controller_flushes_and_restores_state_revision() {
    let root = temp_dir("controller-roundtrip");
    let path = root.join("xtask-state.json");
    let workspace = PathBuf::from("/workspace");
    let mut persistence = XtaskPersistence::new(Some(path.clone()), Some(workspace.clone()));
    let mut state = XtaskState::default();
    state.set_manifest(bool_manifest());
    assert!(state.adjust_selected_arg(1));

    assert!(persistence.flush(&mut state).expect("flush preferences"));
    assert_eq!(state.pending_preferences(), None);

    let restored_persistence = XtaskPersistence::new(Some(path), Some(workspace));
    let mut restored = XtaskState::default();
    restored_persistence
        .restore(&mut restored)
        .expect("restore preferences");
    restored.set_manifest(bool_manifest());

    assert_eq!(
        restored.run_request().expect("run request").args,
        vec!["--all"]
    );
    assert_eq!(restored.pending_preferences(), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stores_preferences_independently_per_workspace() {
    let root = temp_dir("workspace-isolation");
    let path = root.join("xtask-state.json");
    let first = XtaskPersistence::new(Some(path.clone()), Some(PathBuf::from("/workspace/a")));
    let second = XtaskPersistence::new(Some(path.clone()), Some(PathBuf::from("/workspace/b")));

    first
        .save(preferences(
            "release",
            "profile",
            XtaskArgValue::Enum("release".into()),
        ))
        .expect("save first workspace");
    second
        .save(preferences("check", "all", XtaskArgValue::Bool(true)))
        .expect("save second workspace");

    assert_eq!(
        first.load().expect("load first workspace"),
        preferences("release", "profile", XtaskArgValue::Enum("release".into()))
    );
    assert_eq!(
        second.load().expect("load second workspace"),
        preferences("check", "all", XtaskArgValue::Bool(true))
    );
    let stored = fs::read_to_string(&path).expect("read state file");
    assert!(stored.contains(r#""schema_version": 1"#));
    assert!(stored.contains(r#""type": "enum""#));
    assert!(stored.ends_with('\n'));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_preferences_remove_workspace_entry() {
    let root = temp_dir("remove-empty");
    let path = root.join("xtask-state.json");
    let store = XtaskPersistence::new(Some(path.clone()), Some(PathBuf::from("/workspace/a")));
    store
        .save(preferences("check", "all", XtaskArgValue::Bool(true)))
        .expect("save preferences");

    store
        .save(XtaskPreferences::default())
        .expect("remove preferences");

    assert_eq!(
        store.load().expect("load preferences"),
        XtaskPreferences::default()
    );
    let stored = fs::read_to_string(path).expect("read state file");
    assert!(!stored.contains("/workspace/a"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_unknown_state_schema() {
    let root = temp_dir("schema");
    let path = root.join("xtask-state.json");
    fs::create_dir_all(&root).expect("create state directory");
    fs::write(&path, r#"{"schema_version":2,"workspaces":{}}"#).expect("write state file");
    let store = XtaskPersistence::new(Some(path), Some(PathBuf::from("/workspace")));

    let error = store.load().expect_err("unknown schema should fail");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("unsupported xtask state schema 2")
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn state_file_is_user_private() {
    let root = temp_dir("permissions");
    let path = root.join("xtask-state.json");
    let store = XtaskPersistence::new(Some(path.clone()), Some(PathBuf::from("/workspace")));

    store
        .save(preferences("check", "all", XtaskArgValue::Bool(true)))
        .expect("save preferences");

    let mode = fs::metadata(path)
        .expect("state metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let _ = fs::remove_dir_all(root);
}

fn preferences(command: &str, arg: &str, value: XtaskArgValue) -> XtaskPreferences {
    XtaskPreferences {
        values: [(command.to_owned(), [(arg.to_owned(), value)].into())].into(),
    }
}

fn bool_manifest() -> XtaskManifest {
    XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "check".to_owned(),
            about: None,
            args: vec![XtaskArgSpec {
                name: "all".to_owned(),
                long: Some("all".to_owned()),
                short: None,
                help: None,
                required: false,
                value: XtaskValueSpec::Bool { default: false },
            }],
        }],
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "nextdeck-xtask-state-{name}-{}-{suffix}",
        process::id()
    ))
}
