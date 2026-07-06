use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use nextest_metadata::{FilterMatch, TestListSummary};
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
};

use crate::{
    source,
    tree::{DiscoveredTest, TestKey, TestStatus},
};

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
    Binary {
        package: String,
        name: String,
        kind: String,
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
            Self::Binary { name, kind, .. } => format!("{kind} target {name}"),
            Self::Module { path } => format!("module {path}"),
            Self::Test { name } => format!("test {name}"),
            Self::Failed { names } => format!("{} failed test(s)", names.len()),
        }
    }

    pub fn matches_test(&self, test: &DiscoveredTest) -> bool {
        match self {
            Self::Workspace => true,
            Self::Package { name } => test.package == *name,
            Self::Binary {
                package,
                name,
                kind,
            } => test.package == *package && test.binary == *name && test.binary_kind == *kind,
            Self::Module { path } => test.full_name.starts_with(path),
            Self::Test { name } => test.full_name == *name,
            Self::Failed { names } => names.contains(&test.full_name),
        }
    }

    fn nextest_args(&self) -> Vec<String> {
        match self {
            Self::Workspace => Vec::new(),
            Self::Package { name } => vec!["-p".to_owned(), name.clone()],
            Self::Binary {
                package,
                name,
                kind,
            } => binary_nextest_args(package, name, kind),
            Self::Module { path } => vec![path.clone()],
            Self::Test { name } => vec![name.clone()],
            Self::Failed { names } => names.clone(),
        }
    }
}

fn binary_nextest_args(package: &str, name: &str, kind: &str) -> Vec<String> {
    let mut args = vec!["-p".to_owned(), package.to_owned()];
    match kind {
        "lib" => args.push("--lib".to_owned()),
        "test" => args.extend(["--test".to_owned(), name.to_owned()]),
        "bin" => args.extend(["--bin".to_owned(), name.to_owned()]),
        "example" => args.extend(["--example".to_owned(), name.to_owned()]),
        "bench" => args.extend(["--bench".to_owned(), name.to_owned()]),
        _ => {}
    }
    args
}

#[derive(Debug, Clone)]
pub enum RunEvent {
    RunMetadata {
        run_id: Option<String>,
        profile: Option<String>,
    },
    SuiteStarted {
        test_count: usize,
    },
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
    TestOutput {
        key: TestKey,
        stdout: String,
        stderr: String,
    },
    RunnerOutput(String),
    RunnerFinished {
        exit_code: Option<i32>,
    },
    RunnerStopped,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DiscoveryEvent {
    Finished(Result<Vec<DiscoveredTest>, String>),
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

    pub fn project_dir(&self) -> Option<PathBuf> {
        self.manifest_dir()
            .or_else(|| self.current_dir.clone())
    }

    fn manifest_dir(&self) -> Option<PathBuf> {
        let manifest_path = self.manifest_path.as_ref()?;
        let manifest_path = if manifest_path.is_absolute() {
            manifest_path.clone()
        } else if let Some(current_dir) = &self.current_dir {
            current_dir.join(manifest_path)
        } else {
            env::current_dir().ok()?.join(manifest_path)
        };
        manifest_path
            .parent()
            .map(|manifest_dir| cargo_project_root_for_manifest_dir(manifest_dir.to_path_buf()))
    }

    pub async fn discover(&self) -> Result<Vec<DiscoveredTest>> {
        let mut command = self.list_command();
        let output = command
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("running cargo nextest list --message-format=json")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "cargo nextest list exited with {}: {}",
                output.status,
                stderr.trim()
            );
        }

        let summary = serde_json::from_slice::<TestListSummary>(&output.stdout)
            .context("parsing cargo nextest list JSON")?;
        Ok(summary_to_tests(summary))
    }

    pub async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::UnboundedSender<RunEvent>,
        mut stop_rx: mpsc::UnboundedReceiver<()>,
    ) -> Result<()> {
        let mut command = self.run_command(&request);
        let mut child = command
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning cargo nextest run")?;

        let stdout = child.stdout.take().context("nextest stdout unavailable")?;
        let stderr = child.stderr.take().context("nextest stderr unavailable")?;

        let success_output = Arc::new(Mutex::new(SuccessfulOutputCollector::default()));
        let stdout_tx = tx.clone();
        let stdout_success_output = Arc::clone(&success_output);
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if consume_success_output_line(&line, &stdout_success_output, &stdout_tx) {
                    continue;
                }

                match parse_run_line(&line) {
                    Some(event) => {
                        observe_success_output_event(&event, &stdout_success_output);
                        let _ = stdout_tx.send(event);
                    }
                    None if let Some(event) = parse_runner_line(&line) => {
                        let _ = stdout_tx.send(event);
                    }
                    None if !line.trim().is_empty() => {
                        let _ = stdout_tx.send(RunEvent::RunnerOutput(line));
                    }
                    None => {}
                }
            }
            flush_success_output(&stdout_success_output, &stdout_tx);
            anyhow::Ok(())
        });

        let stderr_tx = tx.clone();
        let stderr_success_output = Arc::clone(&success_output);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                if consume_success_output_line(&line, &stderr_success_output, &stderr_tx) {
                    continue;
                }

                if let Some(event) = parse_runner_line(&line) {
                    let _ = stderr_tx.send(event);
                } else if !line.trim().is_empty() {
                    let _ = stderr_tx.send(RunEvent::RunnerOutput(line));
                }
            }
            flush_success_output(&stderr_success_output, &stderr_tx);
            anyhow::Ok(())
        });

        let (status, stopped) = tokio::select! {
            status = child.wait() => {
                (status.context("waiting for nextest")?, false)
            }
            stop = stop_rx.recv() => {
                if stop.is_some() {
                    let _ = tx.send(RunEvent::RunnerOutput("Run stopped by user".to_owned()));
                    if let Err(error) = child.start_kill() {
                        let _ = tx.send(RunEvent::RunnerOutput(format!(
                            "Failed to stop nextest: {error}"
                        )));
                    }
                }
                (
                    child
                        .wait()
                        .await
                        .context("waiting for stopped nextest")?,
                    stop.is_some(),
                )
            }
        };
        stdout_task.await.context("joining stdout task")??;
        stderr_task.await.context("joining stderr task")??;
        if stopped {
            let _ = tx.send(RunEvent::RunnerStopped);
        } else {
            let _ = tx.send(RunEvent::RunnerFinished {
                exit_code: status.code(),
            });
        }
        Ok(())
    }

    fn list_command(&self) -> Command {
        let mut command = Command::new("cargo");
        if let Some(path) = &self.current_dir {
            command.current_dir(path);
        }
        command.args(["nextest", "list", "--message-format", "json"]);
        if let Some(path) = &self.manifest_path {
            command.args(["--manifest-path", &path.to_string_lossy()]);
        }
        command.args(&self.passthrough_args);
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
            "--success-output",
            "immediate",
            "--no-input-handler",
        ]);
        command.args(&self.passthrough_args);
        command.args(request.scope.nextest_args());
        command.env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1");
        command
    }
}

fn cargo_project_root_for_manifest_dir(manifest_dir: PathBuf) -> PathBuf {
    let mut root = None;
    let mut current = Some(manifest_dir.as_path());
    while let Some(dir) = current {
        if manifest_has_workspace_table(&dir.join("Cargo.toml")) {
            root = Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    root.unwrap_or(manifest_dir)
}

fn manifest_has_workspace_table(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    text.lines().any(|line| {
        let line = line.trim();
        line == "[workspace]" || line.starts_with("[workspace.")
    })
}

fn summary_to_tests(summary: TestListSummary) -> Vec<DiscoveredTest> {
    let mut tests = Vec::with_capacity(summary.test_count);
    for (binary_id, suite) in summary.rust_suites {
        let cwd = suite.cwd.as_std_path().to_path_buf();
        let source_path =
            source::binary_source_path(&cwd, suite.binary.kind.as_str(), &suite.binary.binary_name);
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
                binary_kind: suite.binary.kind.as_str().to_owned(),
                cwd: cwd.clone(),
                source_path: source_path.clone(),
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

fn parse_runner_line(line: &str) -> Option<RunEvent> {
    let line = line.trim();
    let rest = line.strip_prefix("Nextest run ID ")?;
    let (run_id, profile) = rest.split_once(" with nextest profile: ")?;
    Some(RunEvent::RunMetadata {
        run_id: Some(run_id.to_owned()),
        profile: Some(profile.to_owned()),
    })
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
        "started" => Some(RunEvent::SuiteStarted {
            test_count: record.test_count.unwrap_or_default(),
        }),
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

#[derive(Default)]
struct SuccessfulOutputCollector {
    last_started: Option<TestKey>,
    last_success: Option<TestKey>,
    collecting_for: Option<TestKey>,
    lines: Vec<String>,
}

impl SuccessfulOutputCollector {
    fn observe_event(&mut self, event: &RunEvent) {
        match event {
            RunEvent::TestStarted { key } => {
                self.last_started = Some(key.clone());
            }
            RunEvent::TestFinished {
                key,
                status: TestStatus::Passed,
                stdout,
                stderr,
                ..
            } if stdout.is_empty() && stderr.is_empty() => {
                self.last_success = Some(key.clone());
            }
            _ => {}
        }
    }

    fn is_collecting(&self) -> bool {
        self.collecting_for.is_some()
    }

    fn try_start(&mut self, line: &str) -> bool {
        if !is_nextest_output_header(line) {
            return false;
        }

        let Some(key) = self
            .last_success
            .take()
            .or_else(|| self.last_started.clone())
        else {
            return false;
        };

        self.collecting_for = Some(key);
        self.lines.clear();
        true
    }

    fn should_finish_before(&self, line: &str) -> bool {
        self.is_collecting()
            && (line.starts_with('{')
                || line.starts_with('─')
                || parse_runner_line(line).is_some()
                || line.trim_start().starts_with("Summary ["))
    }

    fn push_line(&mut self, line: String) {
        self.lines.push(line);
    }

    fn finish_event(&mut self) -> Option<RunEvent> {
        let key = self.collecting_for.take()?;
        let stdout = clean_success_output_block(&key.name, &self.lines);
        self.lines.clear();
        (!stdout.is_empty()).then_some(RunEvent::TestOutput {
            key,
            stdout,
            stderr: String::new(),
        })
    }
}

fn observe_success_output_event(
    event: &RunEvent,
    collector: &Arc<Mutex<SuccessfulOutputCollector>>,
) {
    if let Ok(mut collector) = collector.lock() {
        collector.observe_event(event);
    }
}

fn consume_success_output_line(
    line: &str,
    collector: &Arc<Mutex<SuccessfulOutputCollector>>,
    tx: &mpsc::UnboundedSender<RunEvent>,
) -> bool {
    let mut event = None;
    let consumed = if let Ok(mut collector) = collector.lock() {
        if collector.should_finish_before(line) {
            event = collector.finish_event();
        }

        if collector.is_collecting() {
            collector.push_line(line.to_owned());
            true
        } else {
            collector.try_start(line)
        }
    } else {
        false
    };

    if let Some(event) = event {
        let _ = tx.send(event);
    }
    consumed
}

fn flush_success_output(
    collector: &Arc<Mutex<SuccessfulOutputCollector>>,
    tx: &mpsc::UnboundedSender<RunEvent>,
) {
    let event = collector
        .lock()
        .ok()
        .and_then(|mut collector| collector.finish_event());
    if let Some(event) = event {
        let _ = tx.send(event);
    }
}

fn is_nextest_output_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("output ") && trimmed.contains('─')
}

fn clean_success_output_block(test_name: &str, lines: &[String]) -> String {
    let mut cleaned = lines
        .iter()
        .map(|line| line.strip_prefix("    ").unwrap_or(line).to_owned())
        .filter(|line| !is_libtest_success_output_metadata(test_name, line))
        .collect::<Vec<_>>();
    trim_blank_edges(&mut cleaned);
    cleaned.join("\n")
}

fn is_libtest_success_output_metadata(test_name: &str, line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "running 1 test"
        || trimmed.starts_with("test result: ok.")
        || (trimmed.starts_with("test ")
            && trimmed.contains(test_name)
            && trimmed.ends_with(" ... ok"))
}

fn trim_blank_edges(lines: &mut Vec<String>) {
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
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
mod tests;
