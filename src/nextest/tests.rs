use super::*;

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
    assert!(output.stdout.contains("PASS_STDOUT: lib pass stdout"));
    assert!(output.stdout.contains("PASS_STDERR: lib pass stderr"));
    assert!(!output.stdout.contains("running 1 test"));
    assert!(!output.stdout.contains("test result: ok."));
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn fixture_run_attaches_success_output_from_integration_test_binary() {
    let events = run_output_fixture("integration_pass_prints_output", Vec::new()).await;

    let output = test_output_event(&events, "integration_pass_prints_output");
    assert!(
        output
            .stdout
            .contains("INTEGRATION_STDOUT: integration pass stdout")
    );
    assert!(
        output
            .stdout
            .contains("INTEGRATION_STDERR: integration pass stderr")
    );
}

#[tokio::test]
async fn fixture_run_preserves_child_like_success_output() {
    let events = run_output_fixture("pass_prints_child_like_output", Vec::new()).await;

    let output = test_output_event(&events, "pass_prints_child_like_output");
    assert!(output.stdout.contains("CHILD_STDOUT: command started"));
    assert!(output.stdout.contains("    indented child output"));
    assert!(output.stdout.contains("CHILD_STDERR: command warning"));
}

#[tokio::test]
async fn fixture_run_captures_failed_output_from_json_event() {
    let events = run_output_fixture("fail_prints_stdout_and_stderr", Vec::new()).await;

    let finished = finished_test_event(&events, "fail_prints_stdout_and_stderr");
    assert_eq!(finished.status, TestStatus::Failed);
    assert!(finished.stdout.contains("FAIL_STDOUT: lib fail stdout"));
    assert!(finished.stdout.contains("FAIL_STDERR: lib fail stderr"));
    assert!(
        finished
            .stdout
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
    assert!(output.stdout.contains("IGNORED_STDOUT: ignored stdout"));
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
            stdout,
            stderr,
            duration,
        } => {
            assert_eq!(key.binary_id.as_deref(), Some("demo"));
            assert_eq!(key.event_prefix, None);
            assert_eq!(key.name, "tests::bad");
            assert_eq!(status, TestStatus::Failed);
            assert_eq!(stdout, "out");
            assert_eq!(stderr, "err");
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
        stdout: String::new(),
        stderr: String::new(),
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
            stdout,
            stderr,
        } => {
            assert_eq!(output_key, key);
            assert_eq!(stdout, "stdout from passing test\nstderr from passing test");
            assert_eq!(stderr, "");
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
        stdout: String::new(),
        stderr: String::new(),
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
            stdout,
            stderr,
        } => {
            assert_eq!(output_key, key);
            assert_eq!(
                stdout,
                "capakit-e2e xtask-action=apple-vm:prepare-e2e status=start command=stage-e2e"
            );
            assert_eq!(stderr, "");
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
fn manual_test_command_uses_native_target_flags_and_shell_quoting() {
    let mut test = DiscoveredTest {
        key: TestKey {
            binary_id: Some("demo::demo".to_owned()),
            event_prefix: Some("demo::demo".to_owned()),
            name: "tests::case one".to_owned(),
        },
        package: "demo".to_owned(),
        binary: "demo".to_owned(),
        binary_kind: "lib".to_owned(),
        cwd: PathBuf::from("."),
        source_path: None,
        module: Some("tests".to_owned()),
        name: "case one".to_owned(),
        full_name: "tests::case one".to_owned(),
        status: TestStatus::Pending,
        ignored: true,
    };

    assert_eq!(
        manual_test_command(&test),
        "cargo nextest run -p demo --lib --run-ignored only 'tests::case one'"
    );

    test.binary_kind = "test".to_owned();
    test.binary = "integration".to_owned();
    test.full_name = "tests::case_two".to_owned();
    test.ignored = false;

    assert_eq!(
        manual_test_command(&test),
        "cargo nextest run -p demo --test integration tests::case_two"
    );
}

#[test]
fn manual_run_command_uses_scope_args() {
    assert_eq!(
        manual_run_command(&RunScope::Workspace),
        "cargo nextest run"
    );
    assert_eq!(
        manual_run_command(&RunScope::Package {
            name: "demo".to_owned(),
        }),
        "cargo nextest run -p demo"
    );
    assert_eq!(
        manual_run_command(&RunScope::Module {
            target: target("demo", "demo", "lib"),
            path: "tests::scenario one".to_owned(),
        }),
        "cargo nextest run -p demo --lib 'tests::scenario one'"
    );
    assert_eq!(
        manual_run_command(&RunScope::Failed {
            tests: vec![
                test_selector("alpha", "alpha", "lib", "tests::duplicate_name"),
                test_selector("beta", "beta", "lib", "tests::duplicate_name"),
            ],
        }),
        "cargo nextest run -p alpha --lib tests::duplicate_name && cargo nextest run -p beta --lib tests::duplicate_name"
    );
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

struct CapturedTestOutput {
    stdout: String,
    stderr: String,
}

struct FinishedTestOutput {
    status: TestStatus,
    stdout: String,
}

async fn run_output_fixture(filter: &str, passthrough_args: Vec<String>) -> Vec<RunEvent> {
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
            },
            tx,
            stop_rx,
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
            RunEvent::TestOutput {
                key,
                stdout,
                stderr,
            } if key.name.contains(name) => Some(CapturedTestOutput {
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            }),
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
                stdout,
                stderr,
                ..
            } if key.name.contains(name) => Some(FinishedTestOutput {
                status: *status,
                stdout: format!("{stdout}{stderr}"),
            }),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing TestFinished event for {name}; events: {events:#?}"))
}
