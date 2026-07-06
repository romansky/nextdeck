    use super::*;

    #[test]
    fn project_dir_prefers_manifest_parent_resolved_from_current_dir() {
        let client = NextestClient::new(
            Some(PathBuf::from("crates/demo/Cargo.toml")),
            Some(PathBuf::from("/workspace")),
            Vec::new(),
        );

        assert_eq!(client.project_dir(), Some(PathBuf::from("/workspace/crates/demo")));
    }

    #[test]
    fn project_dir_uses_workspace_root_for_nested_manifest() {
        let root = env::temp_dir().join(format!(
            "nextdeck-nextest-project-dir-{}",
            std::process::id()
        ));
        let package = root.join("crates/demo");
        fs::create_dir_all(&package).expect("create package");
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/demo\"]\n")
            .expect("write workspace manifest");
        fs::write(package.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n")
            .expect("write package manifest");
        let client = NextestClient::new(
            Some(package.join("Cargo.toml")),
            None,
            Vec::new(),
        );

        assert_eq!(client.project_dir(), Some(root.clone()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_dir_uses_current_dir_without_manifest_path() {
        let client = NextestClient::new(
            None,
            Some(PathBuf::from("/workspace")),
            Vec::new(),
        );

        assert_eq!(client.project_dir(), Some(PathBuf::from("/workspace")));
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
        let line =
            " Nextest run ID 5d1f9f3a-f808-42cd-bdf9-3863de01b4d7 with nextest profile: default";
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
            RunScope::Binary {
                package: "demo".to_owned(),
                name: "scenario".to_owned(),
                kind: "test".to_owned(),
            }
            .nextest_args(),
            vec!["-p", "demo", "--test", "scenario"]
        );
        assert_eq!(
            RunScope::Module {
                path: "a::b".to_owned()
            }
            .nextest_args(),
            vec!["a::b"]
        );
        assert_eq!(
            RunScope::Failed {
                names: vec!["a::one".to_owned(), "b::two".to_owned()]
            }
            .nextest_args(),
            vec!["a::one", "b::two"]
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
