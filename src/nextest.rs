use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use nextest_metadata::{FilterMatch, ListCommand, TestListSummary};
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
};

use crate::tree::{DiscoveredTest, TestKey, TestStatus};

#[derive(Clone, Debug, Default)]
pub struct NextestClient {
    manifest_path: Option<PathBuf>,
    current_dir: Option<PathBuf>,
    passthrough_args: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunRequest {
    pub scope: RunScope,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum RunScope {
    #[default]
    Workspace,
    Package {
        name: String,
    },
    Module {
        path: String,
    },
    Test {
        name: String,
    },
    Failed {
        names: Vec<String>,
    },
}

impl RunScope {
    pub fn label(&self) -> String {
        match self {
            Self::Workspace => "workspace".to_owned(),
            Self::Package { name } => format!("package {name}"),
            Self::Module { path } => format!("module {path}"),
            Self::Test { name } => format!("test {name}"),
            Self::Failed { names } => format!("{} failed test(s)", names.len()),
        }
    }

    pub fn matches_test(&self, test: &DiscoveredTest) -> bool {
        match self {
            Self::Workspace => true,
            Self::Package { name } => test.package == *name,
            Self::Module { path } => test.full_name.starts_with(path),
            Self::Test { name } => test.full_name == *name,
            Self::Failed { names } => names.contains(&test.full_name),
        }
    }

    fn nextest_args(&self) -> Vec<String> {
        match self {
            Self::Workspace => Vec::new(),
            Self::Package { name } => vec!["-p".to_owned(), name.clone()],
            Self::Module { path } => vec![path.clone()],
            Self::Test { name } => vec![name.clone()],
            Self::Failed { names } => names.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RunEvent {
    TestStarted {
        key: TestKey,
    },
    TestFinished {
        key: TestKey,
        status: TestStatus,
        stdout: String,
        stderr: String,
        duration: Option<Duration>,
    },
    RunnerOutput(String),
    RunnerFinished {
        exit_code: Option<i32>,
    },
}

impl NextestClient {
    pub fn new(
        manifest_path: Option<PathBuf>,
        current_dir: Option<PathBuf>,
        passthrough_args: Vec<String>,
    ) -> Self {
        Self {
            manifest_path,
            current_dir,
            passthrough_args,
        }
    }

    pub async fn discover(&self) -> Result<Vec<DiscoveredTest>> {
        let client = self.clone();
        let summary = tokio::task::spawn_blocking(move || client.list_command().exec())
            .await
            .context("joining nextest list task")?
            .context("running cargo nextest list --message-format=json")?;
        Ok(summary_to_tests(summary))
    }

    pub async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::UnboundedSender<RunEvent>,
    ) -> Result<()> {
        let mut command = self.run_command(&request);
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning cargo nextest run")?;

        let stdout = child.stdout.take().context("nextest stdout unavailable")?;
        let stderr = child.stderr.take().context("nextest stderr unavailable")?;

        let stdout_tx = tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                match parse_run_line(&line) {
                    Some(event) => {
                        let _ = stdout_tx.send(event);
                    }
                    None if !line.trim().is_empty() => {
                        let _ = stdout_tx.send(RunEvent::RunnerOutput(line));
                    }
                    None => {}
                }
            }
            anyhow::Ok(())
        });

        let stderr_tx = tx.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                if !line.trim().is_empty() {
                    let _ = stderr_tx.send(RunEvent::RunnerOutput(line));
                }
            }
            anyhow::Ok(())
        });

        let status = child.wait().await.context("waiting for nextest")?;
        stdout_task.await.context("joining stdout task")??;
        stderr_task.await.context("joining stderr task")??;
        let _ = tx.send(RunEvent::RunnerFinished {
            exit_code: status.code(),
        });
        Ok(())
    }

    fn list_command(&self) -> ListCommand {
        let mut command = ListCommand::new();
        if let Some(path) = &self.manifest_path {
            command.add_args([
                "--manifest-path".to_owned(),
                path.to_string_lossy().to_string(),
            ]);
        }
        if let Some(path) = &self.current_dir {
            command.current_dir(path.to_string_lossy().to_string());
        }
        command.add_args(self.passthrough_args.clone());
        command
    }

    fn run_command(&self, request: &RunRequest) -> Command {
        let mut command = Command::new("cargo");
        if let Some(path) = &self.current_dir {
            command.current_dir(path);
        }
        command.args(["nextest", "run"]);
        if let Some(path) = &self.manifest_path {
            command.args(["--manifest-path", &path.to_string_lossy()]);
        }
        command.args([
            "--message-format",
            "libtest-json-plus",
            "--message-format-version",
            "0.1",
            "--show-progress",
            "none",
            "--status-level",
            "none",
            "--final-status-level",
            "none",
            "--no-input-handler",
        ]);
        command.args(&self.passthrough_args);
        command.args(request.scope.nextest_args());
        command.env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1");
        command
    }
}

fn summary_to_tests(summary: TestListSummary) -> Vec<DiscoveredTest> {
    let mut tests = Vec::with_capacity(summary.test_count);
    for (binary_id, suite) in summary.rust_suites {
        for (case_name, case) in suite.test_cases {
            let full_name = case_name.as_str().to_owned();
            let (module, name) = case_name.module_path_and_name();
            let status = if case.ignored {
                TestStatus::Ignored
            } else if !matches!(case.filter_match, FilterMatch::Matches) {
                TestStatus::Skipped
            } else {
                TestStatus::Pending
            };

            tests.push(DiscoveredTest {
                key: TestKey {
                    binary_id: Some(binary_id.as_str().to_owned()),
                    event_prefix: Some(format!(
                        "{}::{}",
                        suite.package_name, suite.binary.binary_name
                    )),
                    name: full_name.clone(),
                },
                package: suite.package_name.clone(),
                binary: suite.binary.binary_name.clone(),
                module: module.map(ToOwned::to_owned),
                name: name.to_owned(),
                full_name,
                status,
                ignored: case.ignored,
            });
        }
    }
    tests
}

fn parse_run_line(line: &str) -> Option<RunEvent> {
    let value: Value = serde_json::from_str(line).ok()?;
    let record: LibtestRecord = serde_json::from_value(value.clone()).ok()?;
    match record.record_type.as_deref()? {
        "test" => parse_test_record(value, record),
        "suite" => parse_suite_record(record),
        _ => None,
    }
}

fn parse_test_record(value: Value, record: LibtestRecord) -> Option<RunEvent> {
    let raw_name = record.name?;
    let event_prefix = event_prefix(&raw_name);
    let name = normalize_event_test_name(&raw_name);
    let key = TestKey {
        binary_id: find_string(&value, &["/nextest/binary-id", "/nextest/binary_id"]),
        event_prefix,
        name,
    };

    match record.event.as_deref()? {
        "started" => Some(RunEvent::TestStarted { key }),
        "ok" => Some(RunEvent::TestFinished {
            key,
            status: TestStatus::Passed,
            stdout: record.stdout.unwrap_or_default(),
            stderr: record.stderr.unwrap_or_default(),
            duration: seconds_to_duration(record.exec_time),
        }),
        "failed" => Some(RunEvent::TestFinished {
            key,
            status: TestStatus::Failed,
            stdout: record.stdout.unwrap_or_default(),
            stderr: record.stderr.unwrap_or_default(),
            duration: seconds_to_duration(record.exec_time),
        }),
        "ignored" => Some(RunEvent::TestFinished {
            key,
            status: TestStatus::Ignored,
            stdout: record.stdout.unwrap_or_default(),
            stderr: record.stderr.unwrap_or_default(),
            duration: seconds_to_duration(record.exec_time),
        }),
        "skipped" => Some(RunEvent::TestFinished {
            key,
            status: TestStatus::Skipped,
            stdout: record.stdout.unwrap_or_default(),
            stderr: record.stderr.unwrap_or_default(),
            duration: seconds_to_duration(record.exec_time),
        }),
        _ => None,
    }
}

fn parse_suite_record(record: LibtestRecord) -> Option<RunEvent> {
    match record.event.as_deref()? {
        "started" => Some(RunEvent::RunnerOutput(format!(
            "Starting {} test(s)",
            record.test_count.unwrap_or_default()
        ))),
        "ok" | "failed" => Some(RunEvent::RunnerOutput(format!(
            "Suite finished: {} passed, {} failed, {} ignored, {} filtered out",
            record.passed.unwrap_or_default(),
            record.failed.unwrap_or_default(),
            record.ignored.unwrap_or_default(),
            record.filtered_out.unwrap_or_default()
        ))),
        _ => None,
    }
}

fn find_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .filter_map(|pointer| value.pointer(pointer))
        .find_map(|value| value.as_str().map(ToOwned::to_owned))
}

fn seconds_to_duration(seconds: Option<f64>) -> Option<Duration> {
    seconds.map(Duration::from_secs_f64)
}

fn normalize_event_test_name(name: &str) -> String {
    name.rsplit_once('$')
        .map(|(_, test_name)| test_name)
        .unwrap_or(name)
        .to_owned()
}

fn event_prefix(name: &str) -> Option<String> {
    name.rsplit_once('$')
        .map(|(prefix, _)| prefix.to_owned())
        .filter(|prefix| !prefix.is_empty())
}

#[derive(Debug, Deserialize)]
struct LibtestRecord {
    #[serde(rename = "type")]
    record_type: Option<String>,
    event: Option<String>,
    name: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
    exec_time: Option<f64>,
    test_count: Option<usize>,
    passed: Option<usize>,
    failed: Option<usize>,
    ignored: Option<usize>,
    filtered_out: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let events = include_str!("../tests/fixtures/libtest-json-plus.txt")
            .lines()
            .filter_map(parse_run_line)
            .collect::<Vec<_>>();

        assert_eq!(events.len(), 4);
        match &events[0] {
            RunEvent::RunnerOutput(line) => assert_eq!(line, "Starting 1 test(s)"),
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
                assert_eq!(*status, TestStatus::Passed);
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
}
