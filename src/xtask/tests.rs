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
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "building\n".to_owned(),
        },
    });
    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stderr,
            text: "warning\n".to_owned(),
        },
    });

    let output = state.last_run.as_ref().expect("live output");
    assert!(state.running);
    assert_eq!(output.command_line, "cargo xtask release");
    assert_eq!(output.stdout, "building\n");
    assert_eq!(output.stderr, "warning\n");
    assert!(state.output_text().contains("Running xtask"));
    assert!(state.output_text().contains("building"));
    assert!(state.output_text().contains("warning"));
}

#[test]
fn run_output_uses_output_pane_follow_and_page_size() {
    let mut state = XtaskState::default();
    state.output.set_page_size(2);
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "one\ntwo\nthree\nfour\n".to_owned(),
        },
    });

    assert!(state.output.follow);
    assert!(state.output.scroll > 0);
    let followed_scroll = state.output.scroll;

    state.scroll_output_page_up();

    assert!(!state.output.follow);
    assert_eq!(state.output.scroll, followed_scroll.saturating_sub(2));
    let manual_scroll = state.output.scroll;

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "five\n".to_owned(),
        },
    });

    assert_eq!(state.output.scroll, manual_scroll);

    state.scroll_output_page_down();

    assert_eq!(state.output.scroll, manual_scroll.saturating_add(2));
}

#[test]
fn stale_run_output_is_ignored() {
    let mut state = XtaskState::default();
    let stale_request_id = state.begin_run("cargo xtask old".to_owned());
    let current_request_id = state.begin_run("cargo xtask current".to_owned());

    assert!(!state.apply_event(XtaskEvent::RunOutput {
        request_id: stale_request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "old output\n".to_owned(),
        },
    }));
    assert!(state.apply_event(XtaskEvent::RunOutput {
        request_id: current_request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "current output\n".to_owned(),
        },
    }));

    let output = state.last_run.as_ref().expect("current output");
    assert_eq!(output.command_line, "cargo xtask current");
    assert_eq!(output.stdout, "current output\n");
    assert!(!state.output_text().contains("old output"));
}

#[test]
fn live_run_output_is_bounded() {
    let mut state = XtaskState::default();
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "x".repeat(crate::output::OUTPUT_TEXT_LIMIT_BYTES + 1024),
        },
    });
    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "tail".to_owned(),
        },
    });

    let output = state.last_run.as_ref().expect("live output");
    assert!(output.stdout.len() <= crate::output::OUTPUT_TEXT_LIMIT_BYTES);
    assert!(output.stdout.starts_with("[... output truncated"));
    assert!(output.stdout.ends_with("tail"));
}
