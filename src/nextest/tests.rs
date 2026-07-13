use super::*;

fn output_chunks_text(output: &[TestOutputChunk]) -> String {
    output
        .iter()
        .filter_map(|chunk| match chunk {
            TestOutputChunk::Text(text) => Some(text.as_str()),
            TestOutputChunk::Event(_) => None,
        })
        .collect()
}
use crate::diagnostics::ProcessTracker;

#[test]
fn project_dir_prefers_manifest_parent_resolved_from_current_dir() {
    let client = NextestClient::new(
        Some(PathBuf::from("crates/demo/Cargo.toml")),
        Some(PathBuf::from("/workspace")),
        Vec::new(),
    );

    assert_eq!(
        client.project_dir(),
        Some(PathBuf::from("/workspace/crates/demo"))
    );
}

#[test]
fn project_dir_uses_workspace_root_for_nested_manifest() {
    let root = env::temp_dir().join(format!(
        "nextdeck-nextest-project-dir-{}",
        std::process::id()
    ));
    let package = root.join("crates/demo");
    fs::create_dir_all(&package).expect("create package");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/demo\"]\n",
    )
    .expect("write workspace manifest");
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .expect("write package manifest");
    let client = NextestClient::new(Some(package.join("Cargo.toml")), None, Vec::new());

    assert_eq!(client.project_dir(), Some(root.clone()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_dir_uses_current_dir_without_manifest_path() {
    let client = NextestClient::new(None, Some(PathBuf::from("/workspace")), Vec::new());

    assert_eq!(client.project_dir(), Some(PathBuf::from("/workspace")));
}

#[tokio::test]
async fn fixture_run_attaches_success_output_from_lib_test() {
    let events = run_output_fixture("pass_prints_stdout_and_stderr", Vec::new()).await;

    let output = test_output_event(&events, "pass_prints_stdout_and_stderr");
    assert!(output.text.contains("PASS_STDOUT: lib pass stdout"));
    assert!(output.text.contains("PASS_STDERR: lib pass stderr"));
    assert!(!output.text.contains("running 1 test"));
    assert!(!output.text.contains("test result: ok."));
}

#[tokio::test]
async fn fixture_run_attaches_success_output_from_integration_test_binary() {
    let events = run_output_fixture("integration_pass_prints_output", Vec::new()).await;

    let output = test_output_event(&events, "integration_pass_prints_output");
    assert!(
        output
            .text
            .contains("INTEGRATION_STDOUT: integration pass stdout")
    );
    assert!(
        output
            .text
            .contains("INTEGRATION_STDERR: integration pass stderr")
    );
}

#[tokio::test]
async fn fixture_run_preserves_child_like_success_output() {
    let events = run_output_fixture("pass_prints_child_like_output", Vec::new()).await;

    let output = test_output_event(&events, "pass_prints_child_like_output");
    assert!(output.text.contains("CHILD_STDOUT: command started"));
    assert!(output.text.contains("    indented child output"));
    assert!(output.text.contains("CHILD_STDERR: command warning"));
}

#[tokio::test]
async fn fixture_run_streams_info_preview_before_final_output() {
    let events = run_output_fixture("pass_prints_slow_output_for_info_poll", Vec::new()).await;
    let outputs = test_output_events(&events, "pass_prints_slow_output_for_info_poll");

    assert!(
        outputs.len() >= 2,
        "expected preview and final output chunks; outputs: {outputs:#?}"
    );
    assert!(outputs[0].text.contains("SLOW_PREVIEW: before poll"));
    assert!(!outputs[0].text.contains("SLOW_FINAL: after poll"));
    assert!(
        outputs
            .iter()
            .any(|output| output.text.contains("SLOW_FINAL: after poll"))
    );
    let preview_count = outputs
        .iter()
        .filter(|output| output.text.contains("SLOW_PREVIEW: before poll"))
        .count();
    assert_eq!(preview_count, 1, "outputs: {outputs:#?}");
}

#[tokio::test]
async fn fixture_run_captures_nextdeck_test_events() {
    nextdeck_test_events::event!(
        "running nextdeck event fixture";
        "fixture" => "pass_emits_nextdeck_event",
    );

    let events =
        run_output_fixture_with_events("pass_emits_nextdeck_event", Vec::new(), true).await;

    assert!(
        events.iter().any(|event| matches!(
            event,
            RunEvent::TestOutput { output, .. }
                if output.iter().any(|chunk| matches!(
                    chunk,
                    TestOutputChunk::Event(event)
                        if event.message == "event from fixture"
                            && event.target.as_deref() == Some("fixture")
                            && event.pid.is_some()
                ))
        )),
        "events: {events:#?}"
    );
}

#[tokio::test]
async fn fixture_run_captures_failed_output_from_json_event() {
    let events = run_output_fixture("fail_prints_stdout_and_stderr", Vec::new()).await;

    let finished = finished_test_event(&events, "fail_prints_stdout_and_stderr");
    assert_eq!(finished.status, TestStatus::Failed);
    assert!(finished.output.contains("FAIL_STDOUT: lib fail stdout"));
    assert!(finished.output.contains("FAIL_STDERR: lib fail stderr"));
    assert!(
        finished
            .output
            .contains("FAIL_PANIC: expected fixture failure")
    );
}

#[tokio::test]
async fn fixture_run_can_capture_ignored_test_when_requested() {
    let events = run_output_fixture(
        "ignored_prints_when_explicitly_run",
        vec!["--run-ignored".to_owned(), "only".to_owned()],
    )
    .await;

    let output = test_output_event(&events, "ignored_prints_when_explicitly_run");
    assert!(output.text.contains("IGNORED_STDOUT: ignored stdout"));
}

#[test]
fn parses_libtest_json_plus_started_event() {
    let line = r#"{"type":"test","event":"started","name":"tests::it_works","nextest":{"binary-id":"demo"}}"#;
    let event = parse_run_line(line).expect("event");
    match event {
        RunEvent::TestStarted { key } => {
            assert_eq!(key.binary_id.as_deref(), Some("demo"));
            assert_eq!(key.event_prefix, None);
            assert_eq!(key.name, "tests::it_works");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn parses_nextest_run_metadata_banner() {
    let line = " Nextest run ID 5d1f9f3a-f808-42cd-bdf9-3863de01b4d7 with nextest profile: default";
    let event = parse_runner_line(line).expect("event");
    match event {
        RunEvent::RunMetadata { run_id, profile } => {
            assert_eq!(
                run_id.as_deref(),
                Some("5d1f9f3a-f808-42cd-bdf9-3863de01b4d7")
            );
            assert_eq!(profile.as_deref(), Some("default"));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn strips_current_nextest_binary_prefix_from_test_name() {
    let line = r#"{"type":"test","event":"started","name":"demo::demo_bin$tests::it_works"}"#;
    let event = parse_run_line(line).expect("event");
    match event {
        RunEvent::TestStarted { key } => {
            assert_eq!(key.binary_id, None);
            assert_eq!(key.event_prefix.as_deref(), Some("demo::demo_bin"));
            assert_eq!(key.name, "tests::it_works");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn parses_libtest_json_plus_finished_event() {
    let line = r#"{"type":"test","event":"failed","name":"tests::bad","stdout":"out","stderr":"err","exec_time":0.25,"nextest":{"binary-id":"demo"}}"#;
    let event = parse_run_line(line).expect("event");
    match event {
        RunEvent::TestFinished {
            key,
            status,
            output,
            duration,
        } => {
            assert_eq!(key.binary_id.as_deref(), Some("demo"));
            assert_eq!(key.event_prefix, None);
            assert_eq!(key.name, "tests::bad");
            assert_eq!(status, TestStatus::Failed);
            assert_eq!(output_chunks_text(&output), "out\nerr");
            assert_eq!(duration, Some(Duration::from_millis(250)));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn successful_output_collector_attaches_nextest_output_block_to_last_success() {
    let key = TestKey {
        binary_id: Some("demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::passes".to_owned(),
    };
    let mut collector = SuccessfulOutputCollector::default();
    collector.observe_event(&RunEvent::TestFinished {
        key: key.clone(),
        status: TestStatus::Passed,
        output: Vec::new(),
        duration: Some(Duration::from_millis(5)),
    });

    assert!(collector.try_start("  output ───"));
    collector.push_line(String::new());
    collector.push_line("    running 1 test".to_owned());
    collector.push_line("    stdout from passing test".to_owned());
    collector.push_line("    stderr from passing test".to_owned());
    collector.push_line("    test tests::passes ... ok".to_owned());
    collector.push_line(String::new());
    collector.push_line(
            "    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s"
                .to_owned(),
        );
    collector.push_line(String::new());

    match collector.finish_event().expect("output event") {
        RunEvent::TestOutput {
            key: output_key,
            output,
        } => {
            assert_eq!(output_key, key);
            assert_eq!(
                output_chunks_text(&output),
                "stdout from passing test\nstderr from passing test"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn successful_output_collector_handles_suite_event_before_output_block() {
    let key = TestKey {
        binary_id: None,
        event_prefix: Some("capakit-e2e::bootstrap".to_owned()),
        name: "bootstrap_xtask_prepare_action_is_ready".to_owned(),
    };
    let mut collector = SuccessfulOutputCollector::default();
    collector.observe_event(&RunEvent::TestFinished {
        key: key.clone(),
        status: TestStatus::Passed,
        output: Vec::new(),
        duration: Some(Duration::from_secs(28)),
    });

    let suite_line = r#"{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":28.176076542,"nextest":{"crate":"capakit-e2e","test_binary":"bootstrap","kind":"test"}}"#;
    assert!(parse_run_line(suite_line).is_some());
    assert!(collector.try_start("  output ───"));
    collector.push_line(String::new());
    collector.push_line("    running 1 test".to_owned());
    collector.push_line(
        "    capakit-e2e xtask-action=apple-vm:prepare-e2e status=start command=stage-e2e"
            .to_owned(),
    );
    collector.push_line("    test bootstrap_xtask_prepare_action_is_ready ... ok".to_owned());
    collector.push_line(String::new());
    collector.push_line(
            "    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.16s"
                .to_owned(),
        );

    match collector.finish_event().expect("output event") {
        RunEvent::TestOutput {
            key: output_key,
            output,
        } => {
            assert_eq!(output_key, key);
            assert_eq!(
                output_chunks_text(&output),
                "capakit-e2e xtask-action=apple-vm:prepare-e2e status=start command=stage-e2e"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn successful_output_collector_uses_block_test_name_when_starts_race() {
    let output_key = TestKey {
        binary_id: Some("nextdeck::nextdeck".to_owned()),
        event_prefix: Some("nextdeck::nextdeck".to_owned()),
        name: "output::tests::dogfood_output_captures_stdout_stderr_and_events".to_owned(),
    };
    let later_key = TestKey {
        binary_id: Some("nextdeck::nextdeck".to_owned()),
        event_prefix: Some("nextdeck::nextdeck".to_owned()),
        name: "app::tests::some_later_test".to_owned(),
    };
    let mut collector = SuccessfulOutputCollector::default();
    collector.observe_event(&RunEvent::TestStarted {
        key: output_key.clone(),
    });
    collector.observe_event(&RunEvent::TestStarted { key: later_key });

    assert!(collector.try_start("  output ───"));
    collector.push_line(String::new());
    collector.push_line("    DOGFOOD_OUTPUT stdout before info event".to_owned());
    collector.push_line(
        "    test output::tests::dogfood_output_captures_stdout_stderr_and_events ... ok"
            .to_owned(),
    );

    match collector.finish_event().expect("output event") {
        RunEvent::TestOutput { key, output } => {
            assert_eq!(key, output_key);
            assert_eq!(
                output_chunks_text(&output),
                "DOGFOOD_OUTPUT stdout before info event"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn successful_output_collector_does_not_reuse_key_after_output_was_emitted() {
    let first_key = TestKey {
        binary_id: Some("nextdeck::nextdeck".to_owned()),
        event_prefix: Some("nextdeck::nextdeck".to_owned()),
        name: "output::tests::dogfood_output_captures_stdout_stderr_and_events".to_owned(),
    };
    let next_key = TestKey {
        binary_id: Some("nextdeck::nextdeck".to_owned()),
        event_prefix: Some("nextdeck::nextdeck".to_owned()),
        name: "nextest::tests::later_success".to_owned(),
    };
    let mut collector = SuccessfulOutputCollector::default();
    collector.observe_event(&RunEvent::TestStarted {
        key: first_key.clone(),
    });
    assert!(collector.try_start("  output ───"));
    collector.push_line("    first stdout".to_owned());
    collector.push_line(
        "    test output::tests::dogfood_output_captures_stdout_stderr_and_events ... ok"
            .to_owned(),
    );
    let _ = collector.finish_event().expect("first output event");

    collector.observe_event(&RunEvent::TestFinished {
        key: first_key,
        status: TestStatus::Passed,
        output: Vec::new(),
        duration: Some(Duration::from_millis(5)),
    });
    collector.observe_event(&RunEvent::TestStarted {
        key: next_key.clone(),
    });
    assert!(collector.try_start("  output ───"));
    collector.push_line("    later stdout".to_owned());

    match collector.finish_event().expect("second output event") {
        RunEvent::TestOutput { key, output } => {
            assert_eq!(key, next_key);
            assert_eq!(output_chunks_text(&output), "later stdout");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn successful_output_collector_ignores_unassociated_output_blocks() {
    let mut collector = SuccessfulOutputCollector::default();

    assert!(!collector.try_start("  output ───"));
    assert!(collector.finish_event().is_none());
}

#[test]
fn info_output_collector_emits_deltas_for_all_running_tests() {
    let alpha = TestKey {
        binary_id: Some("sigusr1_probe::sigusr1_probe".to_owned()),
        event_prefix: Some("sigusr1_probe::sigusr1_probe".to_owned()),
        name: "tests::alpha_slow_stdout_and_stderr".to_owned(),
    };
    let beta = TestKey {
        binary_id: Some("sigusr1_probe::sigusr1_probe".to_owned()),
        event_prefix: Some("sigusr1_probe::sigusr1_probe".to_owned()),
        name: "tests::beta_slow_stdout_only".to_owned(),
    };
    let mut collector = InfoOutputCollector::default();
    let mut snapshots = OutputSnapshotTracker::default();
    collector.observe_event(&RunEvent::TestStarted { key: alpha.clone() });
    collector.observe_event(&RunEvent::TestStarted { key: beta.clone() });

    let first = collect_info_events(
        &mut collector,
        &mut snapshots,
        &[
            "────────────",
            "info: 2 running, 0 passed, 0 skipped in 0.100s",
            "",
            "* 1/2:    sigusr1_probe tests::alpha_slow_stdout_and_stderr",
            "  status: test running for 0.100s as PID 10",
            "  output:",
            "",
            "    running 1 test",
            "    alpha stdout start",
            "    alpha stderr start",
            "────────",
            "",
            "* 2/2:    sigusr1_probe tests::beta_slow_stdout_only",
            "  status: test running for 0.099s as PID 11",
            "  output:",
            "",
            "    running 1 test",
            "    beta stdout start",
            "────────────",
        ],
    );

    assert_eq!(
        test_output_text(&first, "alpha"),
        "alpha stdout start\nalpha stderr start"
    );
    assert_eq!(test_output_text(&first, "beta"), "beta stdout start");

    let second = collect_info_events(
        &mut collector,
        &mut snapshots,
        &[
            "info: 2 running, 0 passed, 0 skipped in 0.800s",
            "* 1/2:    sigusr1_probe tests::alpha_slow_stdout_and_stderr",
            "  status: test running for 0.800s as PID 10",
            "  output:",
            "",
            "    running 1 test",
            "    alpha stdout start",
            "    alpha stderr start",
            "    alpha stdout mid",
            "────────────",
        ],
    );

    assert_eq!(second.len(), 1);
    assert_eq!(test_output_text(&second, "alpha"), "alpha stdout mid");
}

#[test]
fn output_snapshot_tracker_dedupes_final_output_after_preview() {
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::passes".to_owned(),
    };
    let mut snapshots = OutputSnapshotTracker::default();

    let preview = snapshots
        .preview_event(key.clone(), "stdout before\nstderr before".to_owned())
        .expect("preview event");
    assert_eq!(
        test_output_text(&[preview], "passes"),
        "stdout before\nstderr before"
    );

    let mut output = vec![TestOutputChunk::Text(
        "stdout before\nstderr before\nstdout after".to_owned(),
    )];
    snapshots.finish_output(&key, &mut output);

    assert_eq!(output_chunks_text(&output), "stdout after");
}

#[test]
fn output_decoder_preserves_text_event_text_order() {
    let event =
        TestEvent::new(nextdeck_test_events::Level::Warn, "cache miss").with_target("cache");
    let json = serde_json::to_string(&event).expect("serialize event");
    let capture = format!(
        "before\n{}{json}\nafter",
        nextdeck_test_events::FRAME_PREFIX
    );
    let chunks = TestOutputDecoder::default().push(&capture, true);

    assert_eq!(
        chunks,
        vec![
            TestOutputChunk::Text("before\n".to_owned()),
            TestOutputChunk::Event(event),
            TestOutputChunk::Text("after".to_owned()),
        ]
    );
}

#[test]
fn output_decoder_retains_a_frame_split_across_polls() {
    let event = TestEvent::new(nextdeck_test_events::Level::Info, "checkpoint");
    let json = serde_json::to_string(&event).expect("serialize event");
    let frame = format!("{}{json}", nextdeck_test_events::FRAME_PREFIX);
    let split = frame.len() - 8;
    let mut decoder = TestOutputDecoder::default();

    assert!(decoder.push(&frame[..split], false).is_empty());
    assert_eq!(
        decoder.push(&frame[split..], false),
        vec![TestOutputChunk::Event(event)]
    );
}

#[test]
fn output_decoder_preserves_plain_text_that_only_looks_like_a_frame() {
    let capture = format!("{}not-json\nafter", nextdeck_test_events::FRAME_PREFIX);

    assert_eq!(
        TestOutputDecoder::default().push(&capture, true),
        vec![TestOutputChunk::Text(capture)]
    );
}

#[test]
fn output_snapshot_tracker_emits_each_framed_event_once() {
    let key = TestKey {
        binary_id: Some("demo::demo".to_owned()),
        event_prefix: Some("demo::demo".to_owned()),
        name: "tests::passes".to_owned(),
    };
    let event = TestEvent::new(nextdeck_test_events::Level::Info, "checkpoint");
    let json = serde_json::to_string(&event).expect("serialize event");
    let first_capture = format!("before\n{}{json}", nextdeck_test_events::FRAME_PREFIX);
    let mut snapshots = OutputSnapshotTracker::default();

    let first = snapshots
        .preview_event(key.clone(), first_capture.clone())
        .expect("first preview");
    let RunEvent::TestOutput { output, .. } = first else {
        panic!("expected test output");
    };
    assert_eq!(
        output,
        vec![
            TestOutputChunk::Text("before\n".to_owned()),
            TestOutputChunk::Event(event),
        ]
    );

    let second = snapshots
        .preview_event(key.clone(), format!("{first_capture}\nafter"))
        .expect("second preview");
    let RunEvent::TestOutput { output, .. } = second else {
        panic!("expected test output");
    };
    assert_eq!(output, vec![TestOutputChunk::Text("after".to_owned())]);

    let mut final_output = vec![TestOutputChunk::Text(format!("{first_capture}\nafter"))];
    snapshots.finish_output(&key, &mut final_output);
    assert!(final_output.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn stop_termination_signals_the_child_process_group() {
    let mut command = Command::new("sh");
    configure_run_command(&mut command);
    command
        .args(["-c", "sleep 30 & wait $!"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().expect("spawn process group fixture");
    let process_group = child.id().expect("child pid");
    tokio::time::sleep(Duration::from_millis(100)).await;

    terminate_child_process_tree(&mut child).expect("terminate process tree");
    let (tx, _rx) = tokio::sync::mpsc::channel(16);
    wait_for_stopped_child(&mut child, &tx)
        .await
        .expect("wait for stopped child");

    for _ in 0..10 {
        if !process_group_exists(process_group) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("process group {process_group} still exists after stop");
}

#[test]
fn scope_args_use_native_package_and_string_filters() {
    assert_eq!(RunScope::Workspace.nextest_args(), Vec::<String>::new());
    assert_eq!(
        RunScope::Package {
            name: "demo".to_owned()
        }
        .nextest_args(),
        vec!["-p", "demo"]
    );
    assert_eq!(
        RunScope::Binary(target("demo", "scenario", "test")).nextest_args(),
        vec!["-p", "demo", "--test", "scenario"]
    );
    assert_eq!(
        RunScope::Module {
            target: target("demo", "demo", "lib"),
            path: "a::b".to_owned()
        }
        .nextest_args(),
        vec!["-p", "demo", "--lib", "a::b"]
    );
    assert_eq!(
        RunScope::Failed {
            tests: vec![
                test_selector("demo", "demo", "lib", "a::one"),
                test_selector("demo", "demo", "lib", "b::two"),
            ]
        }
        .nextest_args(),
        vec!["-p", "demo", "--lib", "a::one", "b::two"]
    );
}

#[test]
fn failed_scope_splits_exact_targets_into_separate_runs() {
    let scope = RunScope::Failed {
        tests: vec![
            test_selector("alpha", "alpha", "lib", "tests::duplicate_name"),
            test_selector("beta", "beta", "lib", "tests::duplicate_name"),
        ],
    };

    assert_eq!(
        scope.nextest_arg_sets(),
        vec![
            vec!["-p", "alpha", "--lib", "tests::duplicate_name"],
            vec!["-p", "beta", "--lib", "tests::duplicate_name"],
        ]
    );
}

#[test]
fn test_and_module_scopes_match_only_their_target() {
    let alpha = discovered_test("alpha", "alpha", "lib", "tests::duplicate_name");
    let beta = discovered_test("beta", "beta", "lib", "tests::duplicate_name");

    let test_scope = RunScope::Test(TestSelector::from_test(&alpha));
    assert!(test_scope.matches_test(&alpha));
    assert!(!test_scope.matches_test(&beta));

    let module_scope = RunScope::Module {
        target: TargetSelector::from_test(&alpha),
        path: "tests".to_owned(),
    };
    assert!(module_scope.matches_test(&alpha));
    assert!(!module_scope.matches_test(&beta));
}

#[test]
fn manual_run_request_command_includes_run_options() {
    let request = RunRequest {
        scope: RunScope::Test(test_selector("demo", "demo", "lib", "tests::case one")),
        options: RunOptions {
            profile: Some("ci".to_owned()),
            filterset: Some("package(demo)".to_owned()),
            ignored: RunIgnored::All,
            retries: Some(2),
            flaky_result: Some(FlakyResult::Fail),
            fail_fast: FailFast::Off,
            max_fail: Some("3:immediate".to_owned()),
            no_capture: true,
            debugger: Some("rust-lldb --args".to_owned()),
            stress_count: Some("10".to_owned()),
            stress_duration: Some("30s".to_owned()),
        },
    };

    assert_eq!(
        manual_run_request_command(&request),
        "cargo nextest run -P ci -E 'package(demo)' --run-ignored all --retries 2 --flaky-result fail --no-fail-fast --max-fail 3:immediate --no-capture --debugger 'rust-lldb --args' --stress-count 10 --stress-duration 30s -p demo --lib 'tests::case one'"
    );
}

#[test]
fn parses_profiles_and_default_filter_presets_from_nextest_config() {
    let profiles = parse_nextest_profiles(
        r#"
            [profile.default]
            default-filter = "not test(slow)"

            [profile.ci]
            retries = 2
            default-filter = "package(nextdeck)"
        "#,
    );

    assert_eq!(
        profiles,
        vec![
            NextestProfile {
                name: "default".to_owned(),
                default_filter: Some("not test(slow)".to_owned()),
            },
            NextestProfile {
                name: "ci".to_owned(),
                default_filter: Some("package(nextdeck)".to_owned()),
            },
        ]
    );
    assert_eq!(
        profile_filter_presets(&profiles),
        vec![
            FilterPreset::Filterset {
                name: "profile default default-filter".to_owned(),
                expression: "not test(slow)".to_owned(),
            },
            FilterPreset::Filterset {
                name: "profile ci default-filter".to_owned(),
                expression: "package(nextdeck)".to_owned(),
            },
        ]
    );
}

#[test]
fn ignored_reason_presets_group_tests_by_reason() {
    let mut first = discovered_test("demo", "demo", "lib", "tests::expensive_one");
    first.ignored = true;
    first.ignore_reason = Some("performance test".to_owned());
    let mut second = discovered_test("demo", "demo", "lib", "tests::expensive_two");
    second.ignored = true;
    second.ignore_reason = Some("performance test".to_owned());

    let presets = ignored_reason_presets(&[first, second]);

    assert_eq!(presets.len(), 1);
    match &presets[0] {
        FilterPreset::IgnoredReason { reason, tests } => {
            assert_eq!(reason, "performance test");
            assert_eq!(tests.len(), 2);
        }
        other => panic!("unexpected preset: {other:?}"),
    }
}

#[test]
fn parses_sampled_libtest_json_plus_fixture() {
    let events = include_str!("../../tests/fixtures/libtest-json-plus.txt")
        .lines()
        .filter_map(parse_run_line)
        .collect::<Vec<_>>();

    assert_eq!(events.len(), 4);
    match &events[0] {
        RunEvent::SuiteStarted { test_count } => assert_eq!(test_count, &1),
        other => panic!("unexpected event: {other:?}"),
    }
    match &events[1] {
        RunEvent::TestStarted { key } => {
            assert_eq!(key.event_prefix.as_deref(), Some("alpha::alpha"));
            assert_eq!(key.name, "tests::duplicate_name");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    match &events[2] {
        RunEvent::TestFinished { key, status, .. } => {
            assert_eq!(key.event_prefix.as_deref(), Some("alpha::alpha"));
            assert_eq!(key.name, "tests::duplicate_name");
            assert_eq!(status, &TestStatus::Passed);
        }
        other => panic!("unexpected event: {other:?}"),
    }
    match &events[3] {
        RunEvent::RunnerOutput(line) => {
            assert_eq!(
                line,
                "Suite finished: 1 passed, 0 failed, 0 ignored, 0 filtered out"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[derive(Debug)]
struct CapturedTestOutput {
    text: String,
}

struct FinishedTestOutput {
    status: TestStatus,
    output: String,
}

async fn run_output_fixture(filter: &str, passthrough_args: Vec<String>) -> Vec<RunEvent> {
    run_output_fixture_with_events(filter, passthrough_args, false).await
}

async fn run_output_fixture_with_events(
    filter: &str,
    passthrough_args: Vec<String>,
    capture_events: bool,
) -> Vec<RunEvent> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/output-workspace");
    let client = NextestClient::new(None, Some(fixture), passthrough_args);
    let (tx, mut rx) = tokio::sync::mpsc::channel(crate::queue::APP_EVENT_QUEUE_CAPACITY);
    let (_stop_tx, stop_rx) = tokio::sync::mpsc::unbounded_channel();
    let (binary, kind) = if filter.starts_with("integration_") {
        ("integration_output", "test")
    } else {
        ("nextdeck_output_fixture", "lib")
    };
    client
        .run(
            RunRequest {
                scope: RunScope::Test(test_selector(
                    "nextdeck-output-fixture",
                    binary,
                    kind,
                    filter,
                )),
                options: RunOptions::default(),
            },
            tx,
            stop_rx,
            capture_events,
            ProcessTracker::default(),
            crate::config::AppSettings::default().test_output_poll_interval(),
        )
        .await
        .expect("run output fixture");

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(
        events.iter().any(|event| matches!(
            event,
            RunEvent::RunnerFinished { .. } | RunEvent::RunnerStopped
        )),
        "fixture run did not finish; events: {events:#?}"
    );
    events
}

fn target(package: &str, name: &str, kind: &str) -> TargetSelector {
    TargetSelector {
        package: package.to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
    }
}

fn test_selector(package: &str, binary: &str, kind: &str, name: &str) -> TestSelector {
    TestSelector {
        target: target(package, binary, kind),
        name: name.to_owned(),
    }
}

fn discovered_test(package: &str, binary: &str, kind: &str, full_name: &str) -> DiscoveredTest {
    DiscoveredTest {
        key: TestKey {
            binary_id: Some(format!("{package}::{binary}")),
            event_prefix: Some(format!("{package}::{binary}")),
            name: full_name.to_owned(),
        },
        package: package.to_owned(),
        binary: binary.to_owned(),
        binary_kind: kind.to_owned(),
        binary_path: PathBuf::from(format!("target/debug/deps/{binary}")),
        cwd: PathBuf::from("."),
        source_path: None,
        module: full_name
            .rsplit_once("::")
            .map(|(module, _)| module.to_owned()),
        name: full_name
            .rsplit_once("::")
            .map(|(_, name)| name)
            .unwrap_or(full_name)
            .to_owned(),
        full_name: full_name.to_owned(),
        status: TestStatus::Pending,
        ignored: false,
        ignore_reason: None,
    }
}

#[cfg(unix)]
fn process_group_exists(process_group: u32) -> bool {
    let result = unsafe { libc::kill(-(process_group as libc::pid_t), 0) };
    if result == 0 {
        return true;
    }

    let error = std::io::Error::last_os_error();
    error.raw_os_error() != Some(libc::ESRCH)
}

fn test_output_event(events: &[RunEvent], name: &str) -> CapturedTestOutput {
    events
        .iter()
        .find_map(|event| match event {
            RunEvent::TestOutput { key, output } if key.name.contains(name) => {
                Some(CapturedTestOutput {
                    text: output_chunks_text(output),
                })
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing TestOutput event for {name}; events: {events:#?}"))
}

fn test_output_events(events: &[RunEvent], name: &str) -> Vec<CapturedTestOutput> {
    events
        .iter()
        .filter_map(|event| match event {
            RunEvent::TestOutput { key, output } if key.name.contains(name) => {
                Some(CapturedTestOutput {
                    text: output_chunks_text(output),
                })
            }
            _ => None,
        })
        .collect()
}

fn collect_info_events(
    collector: &mut InfoOutputCollector,
    snapshots: &mut OutputSnapshotTracker,
    lines: &[&str],
) -> Vec<RunEvent> {
    let mut events = Vec::new();
    for line in lines {
        let (consumed, mut line_events) = collector.consume_line(line, snapshots);
        assert!(consumed, "line was not consumed: {line}");
        events.append(&mut line_events);
    }
    events.extend(collector.finish(snapshots));
    events
}

fn test_output_text(events: &[RunEvent], name: &str) -> String {
    events
        .iter()
        .find_map(|event| match event {
            RunEvent::TestOutput { key, output } if key.name.contains(name) => {
                Some(output_chunks_text(output))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing TestOutput event for {name}; events: {events:#?}"))
}

fn finished_test_event(events: &[RunEvent], name: &str) -> FinishedTestOutput {
    events
        .iter()
        .find_map(|event| match event {
            RunEvent::TestFinished {
                key,
                status,
                output,
                ..
            } if key.name.contains(name) => Some(FinishedTestOutput {
                status: *status,
                output: output_chunks_text(output),
            }),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing TestFinished event for {name}; events: {events:#?}"))
}
