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
fn emits_boolean_flag_when_value_differs_from_declared_default() {
    let mut spec = XtaskArgSpec {
        name: "feature".to_owned(),
        long: Some("feature".to_owned()),
        short: None,
        help: None,
        required: false,
        value: XtaskValueSpec::Bool { default: false },
    };
    let mut args = Vec::new();

    append_arg(&mut args, &spec, &XtaskArgValue::Bool(true)).expect("set true flag");
    assert_eq!(args, ["--feature"]);

    spec.value = XtaskValueSpec::Bool { default: true };
    args.clear();
    append_arg(&mut args, &spec, &XtaskArgValue::Bool(false)).expect("set false flag");
    assert_eq!(args, ["--feature"]);
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
fn manifest_refresh_preserves_session_arg_values() {
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

    state.set_manifest(sample_manifest());

    let request = state.run_request().expect("run request");
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
}

#[test]
fn manifest_refresh_drops_values_that_no_longer_match_spec() {
    let mut state = XtaskState::default();
    state.set_manifest(sample_manifest());
    state.select_next_arg();
    state.select_next_arg();
    state.select_next_arg();
    state.adjust_selected_arg(1);

    let mut manifest = sample_manifest();
    if let XtaskValueSpec::Enum { values, default } = &mut manifest.commands[0].args[3].value {
        *values = vec!["debug".to_owned()];
        *default = Some("debug".to_owned());
    }

    state.set_manifest(manifest);

    let request = state.run_request().expect("run request");
    assert!(!request.args.iter().any(|arg| arg == "--profile"));
}

#[test]
fn restored_preferences_apply_without_becoming_dirty() {
    let mut state = XtaskState::default();
    state.restore_preferences(XtaskPreferences {
        values: [(
            "release".to_owned(),
            [(
                "profile".to_owned(),
                XtaskArgValue::Enum("release".to_owned()),
            )]
            .into(),
        )]
        .into(),
    });

    state.set_manifest(sample_manifest());

    assert_eq!(
        state.run_request().expect("run request").args,
        vec!["--profile", "release"]
    );
    assert_eq!(state.pending_preferences(), None);
}

#[test]
fn restored_stale_preferences_are_pruned_and_marked_for_persistence() {
    let mut state = XtaskState::default();
    state.restore_preferences(XtaskPreferences {
        values: [(
            "release".to_owned(),
            [(
                "profile".to_owned(),
                XtaskArgValue::Enum("removed".to_owned()),
            )]
            .into(),
        )]
        .into(),
    });

    state.set_manifest(sample_manifest());

    assert!(state.run_request().expect("run request").args.is_empty());
    let (revision, preferences) = state.pending_preferences().expect("pending cleanup");
    assert_eq!(revision, 1);
    assert!(preferences.is_empty());
}

#[test]
fn returning_to_manifest_default_removes_persisted_override() {
    let mut state = XtaskState::default();
    state.set_manifest(sample_manifest());

    assert!(state.adjust_selected_arg(1));
    let (first_revision, preferences) = state.pending_preferences().expect("bool override");
    assert_eq!(
        preferences.value("release", "allow-dirty"),
        Some(&XtaskArgValue::Bool(true))
    );
    state.mark_preferences_persisted(first_revision);

    assert!(state.adjust_selected_arg(-1));
    let (_, preferences) = state.pending_preferences().expect("default cleanup");
    assert!(preferences.is_empty());
}

#[test]
fn parameter_viewport_follows_selected_arg_lines() {
    let mut state = XtaskState::default();
    state.set_manifest(XtaskManifest {
        schema_version: SCHEMA_VERSION,
        commands: vec![XtaskCommandSpec {
            name: "ship".to_owned(),
            about: None,
            args: (0..12)
                .map(|index| XtaskArgSpec {
                    name: format!("arg-{index}"),
                    long: None,
                    short: None,
                    help: Some(format!("Argument {index}")),
                    required: false,
                    value: XtaskValueSpec::String { default: None },
                })
                .collect(),
        }],
    });
    state.open_detail();
    state.apply_parameters_viewport_metrics(5);

    for _ in 0..8 {
        state.select_next_arg();
    }

    let (selected_line, selected_len, _) = state.selected_parameter_range().expect("selected line");
    let scroll = state.parameters_viewport.scroll();
    assert!(scroll > 0);
    assert!(selected_line >= scroll);
    assert!(selected_line + selected_len <= scroll + state.parameters_viewport.page_size());
}

#[test]
fn parameter_page_size_refresh_preserves_manual_scroll() {
    let mut state = XtaskState::default();
    state.set_manifest(sample_manifest());
    state.open_detail();
    state.apply_parameters_viewport_metrics(5);
    state
        .parameters_viewport
        .apply_scroll(crate::scroll::ScrollAction::PageDown);
    let manual_scroll = state.parameters_viewport.scroll();

    state.apply_parameters_viewport_metrics(5);

    assert!(manual_scroll > 0);
    assert_eq!(state.parameters_viewport.scroll(), manual_scroll);
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
    assert_eq!(output.combined, "building\nwarning\n");
    assert_eq!(output.stdout, "building\n");
    assert_eq!(output.stderr, "warning\n");
    let text = state.output_text();
    assert!(text.contains("Running xtask"));
    assert!(text.contains("building\nwarning"));
    assert!(!text.contains("\nstdout\n"));
    assert!(!text.contains("\nstderr\n"));
}

#[test]
fn output_pane_is_shared_between_commands() {
    let mut manifest = sample_manifest();
    let mut check = manifest.commands[0].clone();
    check.name = "check".to_owned();
    manifest.commands.push(check);

    let mut state = XtaskState::default();
    state.set_manifest(manifest);
    assert!(state.open_detail());
    state.output.apply_viewport_page_size(2);

    let request_id = state.begin_run("cargo xtask release".to_owned());
    assert!(state.apply_event(XtaskEvent::RunFinished {
        request_id,
        result: Ok(XtaskRunOutput {
            command_line: "cargo xtask release".to_owned(),
            success: true,
            exit_code: Some(0),
            combined: "one\ntwo\nthree\nfour\n".to_owned(),
            stdout: "one\ntwo\nthree\nfour\n".to_owned(),
            stderr: String::new(),
        }),
    }));
    let output_text = state.output_text();
    let line_count = state.output.output_view(&output_text).line_count();
    state
        .output
        .apply_scroll(crate::scroll::ScrollAction::PageUp, line_count);
    let scroll = state.output.scroll();
    assert!(!state.output.follow());

    state.close_detail();
    state.select_next_command();
    assert!(state.open_detail());

    assert_eq!(
        state
            .selected_command()
            .map(|command| command.name.as_str()),
        Some("check")
    );
    assert_eq!(state.output_text(), output_text);
    assert_eq!(state.output.scroll(), scroll);
    assert!(!state.output.follow());
}

#[test]
fn run_finished_preserves_live_interleaved_output() {
    let mut state = XtaskState::default();
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "stdout 1\n".to_owned(),
        },
    });
    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stderr,
            text: "stderr 1\n".to_owned(),
        },
    });
    state.apply_event(XtaskEvent::RunFinished {
        request_id,
        result: Ok(XtaskRunOutput {
            command_line: "cargo xtask release".to_owned(),
            success: true,
            exit_code: Some(0),
            combined: String::new(),
            stdout: "stdout 1\n".to_owned(),
            stderr: "stderr 1\n".to_owned(),
        }),
    });

    let output = state.last_run.as_ref().expect("finished output");
    assert!(!state.running);
    assert_eq!(output.combined, "stdout 1\nstderr 1\n");
    assert!(state.output_text().contains("stdout 1\nstderr 1"));
    assert!(!state.output_text().contains("\nstdout\n"));
    assert!(!state.output_text().contains("\nstderr\n"));
}

#[test]
fn run_finished_without_live_chunks_falls_back_to_single_stream() {
    let mut state = XtaskState::default();
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunFinished {
        request_id,
        result: Ok(XtaskRunOutput {
            command_line: "cargo xtask release".to_owned(),
            success: false,
            exit_code: Some(1),
            combined: String::new(),
            stdout: "stdout tail".to_owned(),
            stderr: "stderr tail\n".to_owned(),
        }),
    });

    let output = state.last_run.as_ref().expect("finished output");
    assert_eq!(output.combined, "stdout tail\nstderr tail\n");
    assert!(state.output_text().contains("stdout tail\nstderr tail"));
    assert!(!state.output_text().contains("\nstdout\n"));
    assert!(!state.output_text().contains("\nstderr\n"));
}

#[test]
fn run_output_uses_output_pane_follow_and_page_size() {
    let mut state = XtaskState::default();
    state.output.apply_viewport_page_size(2);
    let request_id = state.begin_run("cargo xtask release".to_owned());

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "one\ntwo\nthree\nfour\n".to_owned(),
        },
    });

    assert!(state.output.follow());
    assert!(state.output.scroll() > 0);
    let followed_scroll = state.output.scroll();

    let line_count = state.output.output_view(&state.output_text()).line_count();
    state
        .output
        .apply_scroll(crate::scroll::ScrollAction::PageUp, line_count);

    assert!(!state.output.follow());
    assert_eq!(state.output.scroll(), followed_scroll.saturating_sub(2));
    let manual_scroll = state.output.scroll();

    state.apply_event(XtaskEvent::RunOutput {
        request_id,
        chunk: XtaskRunChunk {
            stream: XtaskOutputStream::Stdout,
            text: "five\n".to_owned(),
        },
    });

    assert_eq!(state.output.scroll(), manual_scroll);

    let line_count = state.output.output_view(&state.output_text()).line_count();
    state
        .output
        .apply_scroll(crate::scroll::ScrollAction::PageDown, line_count);

    assert_eq!(state.output.scroll(), manual_scroll.saturating_add(2));
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
    assert_eq!(output.combined, "current output\n");
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
    assert!(output.combined.len() <= crate::output::OUTPUT_TEXT_LIMIT_BYTES);
    assert!(output.combined.starts_with("[... output truncated"));
    assert!(output.combined.ends_with("tail"));
    assert!(output.stdout.len() <= crate::output::OUTPUT_TEXT_LIMIT_BYTES);
    assert!(output.stdout.starts_with("[... output truncated"));
    assert!(output.stdout.ends_with("tail"));
}
