use super::*;

fn sample_manifest() -> XtaskManifest {
    XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "release".to_owned(),
            about: Some("Build a release".to_owned()),
            args: vec![
                XtaskArgSpec {
                    name: "allow-dirty".to_owned(),
                    long: Some("allow-dirty".to_owned()),
                    short: None,
                    help: Some("Allow dirty worktree".to_owned()),
                    required: false,
                    value: XtaskValueSpec::Bool { default: false },
                },
                XtaskArgSpec {
                    name: "version".to_owned(),
                    long: Some("version".to_owned()),
                    short: None,
                    help: Some("Release version".to_owned()),
                    required: false,
                    value: XtaskValueSpec::String { default: None },
                },
                XtaskArgSpec {
                    name: "retries".to_owned(),
                    long: Some("retries".to_owned()),
                    short: None,
                    help: Some("Retry count".to_owned()),
                    required: false,
                    value: XtaskValueSpec::Number { default: Some(1) },
                },
                XtaskArgSpec {
                    name: "profile".to_owned(),
                    long: Some("profile".to_owned()),
                    short: None,
                    help: Some("Release profile".to_owned()),
                    required: false,
                    value: XtaskValueSpec::Enum {
                        values: vec!["debug".to_owned(), "release".to_owned()],
                        default: Some("debug".to_owned()),
                    },
                },
            ],
        }],
    }
}

#[test]
fn parses_and_validates_manifest() {
    let json = r#"
      {
        "schema_version": 1,
        "commands": [
          {
            "name": "check",
            "about": "Run checks",
            "args": [
              {
                "name": "allow-dirty",
                "help": "Allow dirty worktree",
                "value": { "type": "bool", "default": false }
              }
            ]
          }
        ]
      }
    "#;

    let manifest = serde_json::from_str::<XtaskManifest>(json).expect("manifest");

    manifest.validate().expect("valid manifest");
    assert_eq!(manifest.commands[0].args[0].flag(), "--allow-dirty");
}

#[test]
fn builds_run_request_from_changed_values() {
    let mut state = XtaskState::default();
    state.set_manifest(sample_manifest());
    state.adjust_selected_arg(1);
    state.select_next_arg();
    state.begin_edit_selected_arg();
    state.edit_input(InputFieldInput::char('1'));
    state.edit_input(InputFieldInput::char('.'));
    state.edit_input(InputFieldInput::char('2'));
    state.edit_input(InputFieldInput::char('.'));
    state.edit_input(InputFieldInput::char('3'));
    state.commit_edit().expect("commit version");
    state.select_next_arg();
    state.adjust_selected_arg(1);
    state.select_next_arg();
    state.adjust_selected_arg(1);

    let request = state.run_request().expect("run request");

    assert_eq!(request.command, "release");
    assert_eq!(
        request.args,
        vec![
            "--allow-dirty",
            "--version",
            "1.2.3",
            "--retries",
            "2",
            "--profile",
            "release"
        ]
    );
    assert_eq!(
        request.command_line(),
        "cargo xtask release --allow-dirty --version 1.2.3 --retries 2 --profile release"
    );
}

#[test]
fn omits_optional_defaults() {
    let mut state = XtaskState::default();
    state.set_manifest(sample_manifest());

    let request = state.run_request().expect("run request");

    assert_eq!(request.args, Vec::<String>::new());
}

#[test]
fn validates_required_values() {
    let mut manifest = sample_manifest();
    manifest.commands[0].args[1].required = true;
    let mut state = XtaskState::default();
    state.set_manifest(manifest);

    let error = state
        .run_request()
        .expect_err("missing version should fail");

    assert!(error.to_string().contains("version is required"));
}

#[test]
fn rejects_unknown_enum_defaults() {
    let manifest = XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "bad".to_owned(),
            about: None,
            args: vec![XtaskArgSpec {
                name: "mode".to_owned(),
                long: None,
                short: None,
                help: None,
                required: false,
                value: XtaskValueSpec::Enum {
                    values: vec!["one".to_owned()],
                    default: Some("two".to_owned()),
                },
            }],
        }],
    };

    let error = manifest.validate().expect_err("invalid enum default");

    assert!(error.to_string().contains("default is not in values"));
}

#[test]
fn live_run_output_appends_while_running() {
    let mut state = XtaskState::default();
    state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput(XtaskRunChunk {
        stream: XtaskOutputStream::Stdout,
        text: "building\n".to_owned(),
    }));
    state.apply_event(XtaskEvent::RunOutput(XtaskRunChunk {
        stream: XtaskOutputStream::Stderr,
        text: "warning\n".to_owned(),
    }));

    let output = state.last_run.as_ref().expect("live output");
    assert!(state.running);
    assert_eq!(output.command_line, "cargo xtask release");
    assert_eq!(output.stdout, "building\n");
    assert_eq!(output.stderr, "warning\n");
    assert!(state.output_text().contains("Running xtask"));
    assert!(state.output_text().contains("building"));
    assert!(state.output_text().contains("warning"));
}
