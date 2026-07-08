use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use nextdeck_test_events::TestEvent;
use nextest_metadata::{FilterMatch, TestListSummary};
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::mpsc,
    time,
};

use crate::{
    diagnostics::ProcessTracker,
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
    pub options: RunOptions,
}

impl RunRequest {
    pub fn new(scope: RunScope) -> Self {
        Self {
            scope,
            options: RunOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum RunScope {
    #[default]
    Workspace,
    Package {
        name: String,
    },
    Binary(TargetSelector),
    Module {
        target: TargetSelector,
        path: String,
    },
    Test(TestSelector),
    Failed {
        tests: Vec<TestSelector>,
    },
    TestSet {
        label: String,
        tests: Vec<TestSelector>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunOptions {
    pub profile: Option<String>,
    pub filterset: Option<String>,
    pub ignored: RunIgnored,
    pub retries: Option<u32>,
    pub flaky_result: Option<FlakyResult>,
    pub fail_fast: FailFast,
    pub max_fail: Option<String>,
    pub no_capture: bool,
    pub debugger: Option<String>,
    pub stress_count: Option<String>,
    pub stress_duration: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RunIgnored {
    #[default]
    Default,
    Only,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlakyResult {
    Pass,
    Fail,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FailFast {
    #[default]
    Profile,
    On,
    Off,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryOutput {
    pub tests: Vec<DiscoveredTest>,
    pub run_config: RunConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunConfig {
    pub profiles: Vec<NextestProfile>,
    pub filter_presets: Vec<FilterPreset>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            profiles: vec![NextestProfile {
                name: "default".to_owned(),
                default_filter: None,
            }],
            filter_presets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NextestProfile {
    pub name: String,
    pub default_filter: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterPreset {
    Filterset {
        name: String,
        expression: String,
    },
    IgnoredReason {
        reason: String,
        tests: Vec<TestSelector>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TargetSelector {
    pub package: String,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TestSelector {
    pub target: TargetSelector,
    pub name: String,
}

impl TargetSelector {
    pub fn from_test(test: &DiscoveredTest) -> Self {
        Self {
            package: test.package.clone(),
            name: test.binary.clone(),
            kind: test.binary_kind.clone(),
        }
    }

    fn matches_test(&self, test: &DiscoveredTest) -> bool {
        test.package == self.package && test.binary == self.name && test.binary_kind == self.kind
    }

    fn nextest_args(&self) -> Vec<String> {
        binary_nextest_args(&self.package, &self.name, &self.kind)
    }
}

impl TestSelector {
    pub fn from_test(test: &DiscoveredTest) -> Self {
        Self {
            target: TargetSelector::from_test(test),
            name: test.full_name.clone(),
        }
    }

    fn matches_test(&self, test: &DiscoveredTest) -> bool {
        self.target.matches_test(test) && test.full_name == self.name
    }
}

impl RunOptions {
    fn nextest_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(profile) = non_empty_option(&self.profile)
            && profile != "default"
        {
            args.extend(["-P".to_owned(), profile.to_owned()]);
        }
        if let Some(filterset) = non_empty_option(&self.filterset) {
            args.extend(["-E".to_owned(), filterset.to_owned()]);
        }
        match self.ignored {
            RunIgnored::Default => {}
            RunIgnored::Only => args.extend(["--run-ignored".to_owned(), "only".to_owned()]),
            RunIgnored::All => args.extend(["--run-ignored".to_owned(), "all".to_owned()]),
        }
        if let Some(retries) = self.retries {
            args.extend(["--retries".to_owned(), retries.to_string()]);
        }
        if let Some(flaky_result) = self.flaky_result {
            args.extend([
                "--flaky-result".to_owned(),
                flaky_result.nextest_value().to_owned(),
            ]);
        }
        match self.fail_fast {
            FailFast::Profile => {}
            FailFast::On => args.push("--fail-fast".to_owned()),
            FailFast::Off => args.push("--no-fail-fast".to_owned()),
        }
        if let Some(max_fail) = non_empty_option(&self.max_fail) {
            args.extend(["--max-fail".to_owned(), max_fail.to_owned()]);
        }
        if self.no_capture {
            args.push("--no-capture".to_owned());
        }
        if let Some(debugger) = non_empty_option(&self.debugger) {
            args.extend(["--debugger".to_owned(), debugger.to_owned()]);
        }
        if let Some(stress_count) = non_empty_option(&self.stress_count) {
            args.extend(["--stress-count".to_owned(), stress_count.to_owned()]);
        }
        if let Some(stress_duration) = non_empty_option(&self.stress_duration) {
            args.extend(["--stress-duration".to_owned(), stress_duration.to_owned()]);
        }
        args
    }
}

impl RunIgnored {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Only => "only ignored",
            Self::All => "all",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Default => Self::Only,
            Self::Only => Self::All,
            Self::All => Self::Default,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Default => Self::All,
            Self::Only => Self::Default,
            Self::All => Self::Only,
        }
    }
}

impl FlakyResult {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }

    const fn nextest_value(self) -> &'static str {
        self.label()
    }
}

impl FailFast {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Profile => Self::On,
            Self::On => Self::Off,
            Self::Off => Self::Profile,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Profile => Self::Off,
            Self::On => Self::Profile,
            Self::Off => Self::On,
        }
    }
}

impl FilterPreset {
    pub fn name(&self) -> &str {
        match self {
            Self::Filterset { name, .. } => name,
            Self::IgnoredReason { reason, .. } => reason,
        }
    }
}

fn non_empty_option(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

impl RunScope {
    pub fn label(&self) -> String {
        match self {
            Self::Workspace => "workspace".to_owned(),
            Self::Package { name } => format!("package {name}"),
            Self::Binary(target) => format!("{} target {}", target.kind, target.name),
            Self::Module { path, .. } => format!("module {path}"),
            Self::Test(test) => format!("test {}", test.name),
            Self::Failed { tests } => format!("{} failed test(s)", tests.len()),
            Self::TestSet { label, .. } => label.clone(),
        }
    }

    pub fn matches_test(&self, test: &DiscoveredTest) -> bool {
        match self {
            Self::Workspace => true,
            Self::Package { name } => test.package == *name,
            Self::Binary(target) => target.matches_test(test),
            Self::Module { target, path } => {
                target.matches_test(test)
                    && (test.full_name == *path
                        || test
                            .full_name
                            .strip_prefix(path)
                            .is_some_and(|rest| rest.starts_with("::")))
            }
            Self::Test(selector) => selector.matches_test(test),
            Self::Failed { tests } => tests.iter().any(|selector| selector.matches_test(test)),
            Self::TestSet { tests, .. } => tests.iter().any(|selector| selector.matches_test(test)),
        }
    }

    #[cfg(test)]
    fn nextest_args(&self) -> Vec<String> {
        self.nextest_arg_sets()
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    fn nextest_arg_sets(&self) -> Vec<Vec<String>> {
        match self {
            Self::Workspace => vec![Vec::new()],
            Self::Package { name } => vec![vec!["-p".to_owned(), name.clone()]],
            Self::Binary(target) => vec![target.nextest_args()],
            Self::Module { target, path } => {
                let mut args = target.nextest_args();
                args.push(path.clone());
                vec![args]
            }
            Self::Test(test) => {
                let mut args = test.target.nextest_args();
                args.push(test.name.clone());
                vec![args]
            }
            Self::Failed { tests } => failed_nextest_arg_sets(tests),
            Self::TestSet { tests, .. } => grouped_test_arg_sets(tests),
        }
    }
}

fn failed_nextest_arg_sets(tests: &[TestSelector]) -> Vec<Vec<String>> {
    grouped_test_arg_sets(tests)
}

fn grouped_test_arg_sets(tests: &[TestSelector]) -> Vec<Vec<String>> {
    let mut grouped: BTreeMap<TargetSelector, Vec<String>> = BTreeMap::new();
    for test in tests {
        let names = grouped.entry(test.target.clone()).or_default();
        if !names.contains(&test.name) {
            names.push(test.name.clone());
        }
    }

    grouped
        .into_iter()
        .map(|(target, names)| {
            let mut args = target.nextest_args();
            args.extend(names);
            args
        })
        .collect()
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

pub fn manual_run_request_command(request: &RunRequest) -> String {
    request
        .scope
        .nextest_arg_sets()
        .into_iter()
        .map(|scope_args| {
            let mut args = vec!["cargo".to_owned(), "nextest".to_owned(), "run".to_owned()];
            args.extend(request.options.nextest_args());
            args.extend(scope_args);
            shell_command(args)
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

fn shell_command(args: Vec<String>) -> String {
    args.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
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
    TestEvent {
        run_id: String,
        event: TestEvent,
    },
    RunnerOutput(String),
    RunnerFinished {
        exit_code: Option<i32>,
    },
    RunnerStopped,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DiscoveryEvent {
    Finished(Result<DiscoveryOutput, String>),
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
        self.manifest_dir().or_else(|| self.current_dir.clone())
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

    pub async fn discover(&self) -> Result<DiscoveryOutput> {
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
        let mut tests = summary_to_tests(summary);
        annotate_ignore_reasons(&mut tests);
        let run_config = self.discover_run_config(&tests);
        Ok(DiscoveryOutput { tests, run_config })
    }

    pub async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<RunEvent>,
        mut stop_rx: mpsc::UnboundedReceiver<()>,
        test_events_path: Option<PathBuf>,
        process_tracker: ProcessTracker,
    ) -> Result<()> {
        let arg_sets = request.scope.nextest_arg_sets();
        let total_runs = arg_sets.len();
        let mut exit_code = Some(0);
        for (index, scope_args) in arg_sets.into_iter().enumerate() {
            if total_runs > 1 {
                let _ = tx
                    .send(RunEvent::RunnerOutput(format!(
                        "Starting test group {}/{}",
                        index + 1,
                        total_runs
                    )))
                    .await;
            }

            match self
                .run_once(
                    scope_args,
                    &request.options,
                    &tx,
                    &mut stop_rx,
                    test_events_path.as_ref(),
                    &process_tracker,
                )
                .await?
            {
                RunProcessOutcome::Finished(status) => {
                    if !status.success() && exit_code == Some(0) {
                        exit_code = status.code().or(Some(1));
                    }
                }
                RunProcessOutcome::Stopped => {
                    let _ = tx.send(RunEvent::RunnerStopped).await;
                    return Ok(());
                }
            }
        }

        let _ = tx.send(RunEvent::RunnerFinished { exit_code }).await;
        Ok(())
    }

    async fn run_once(
        &self,
        scope_args: Vec<String>,
        options: &RunOptions,
        tx: &mpsc::Sender<RunEvent>,
        stop_rx: &mut mpsc::UnboundedReceiver<()>,
        test_events_path: Option<&PathBuf>,
        process_tracker: &ProcessTracker,
    ) -> Result<RunProcessOutcome> {
        let mut command = self.run_command(scope_args, options, test_events_path);
        configure_run_command(&mut command);
        let mut child = command
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning cargo nextest run")?;
        process_tracker.set(child.id());

        let stdout = child.stdout.take().context("nextest stdout unavailable")?;
        let stderr = child.stderr.take().context("nextest stderr unavailable")?;

        let success_output = Arc::new(Mutex::new(SuccessfulOutputCollector::default()));
        let stdout_tx = tx.clone();
        let stdout_success_output = Arc::clone(&success_output);
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if consume_success_output_line(&line, &stdout_success_output, &stdout_tx).await {
                    continue;
                }

                match parse_run_line(&line) {
                    Some(event) => {
                        observe_success_output_event(&event, &stdout_success_output);
                        let _ = stdout_tx.send(event).await;
                    }
                    None if let Some(event) = parse_runner_line(&line) => {
                        let _ = stdout_tx.send(event).await;
                    }
                    None if !line.trim().is_empty() => {
                        let _ = stdout_tx.send(RunEvent::RunnerOutput(line)).await;
                    }
                    None => {}
                }
            }
            flush_success_output(&stdout_success_output, &stdout_tx).await;
            anyhow::Ok(())
        });

        let stderr_tx = tx.clone();
        let stderr_success_output = Arc::clone(&success_output);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                if consume_success_output_line(&line, &stderr_success_output, &stderr_tx).await {
                    continue;
                }

                if let Some(event) = parse_runner_line(&line) {
                    let _ = stderr_tx.send(event).await;
                } else if !line.trim().is_empty() {
                    let _ = stderr_tx.send(RunEvent::RunnerOutput(line)).await;
                }
            }
            flush_success_output(&stderr_success_output, &stderr_tx).await;
            anyhow::Ok(())
        });

        let (status, stopped) = tokio::select! {
            status = child.wait() => {
                (status.context("waiting for nextest")?, false)
            }
            stop = stop_rx.recv() => {
                if stop.is_some() {
                    let _ = tx.send(RunEvent::RunnerOutput("Run stopped by user".to_owned())).await;
                    if let Err(error) = terminate_child_process_tree(&mut child) {
                        let _ = tx.send(RunEvent::RunnerOutput(format!(
                            "Failed to stop nextest: {error}"
                        ))).await;
                    }
                    (
                        wait_for_stopped_child(&mut child, tx)
                            .await?,
                        true,
                    )
                } else {
                    (
                        child
                            .wait()
                            .await
                            .context("waiting for nextest after stop channel closed")?,
                        false,
                    )
                }
            }
        };
        process_tracker.clear();
        stdout_task.await.context("joining stdout task")??;
        stderr_task.await.context("joining stderr task")??;
        if stopped {
            Ok(RunProcessOutcome::Stopped)
        } else {
            Ok(RunProcessOutcome::Finished(status))
        }
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

    fn run_command(
        &self,
        scope_args: Vec<String>,
        options: &RunOptions,
        test_events_path: Option<&PathBuf>,
    ) -> Command {
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
        command.args(options.nextest_args());
        command.args(scope_args);
        command.env("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1");
        if let Some(path) = test_events_path {
            command.env(nextdeck_test_events::ENV_VAR, path);
        }
        command
    }

    fn discover_run_config(&self, tests: &[DiscoveredTest]) -> RunConfig {
        let profiles = self
            .project_dir()
            .and_then(|project_dir| {
                read_nextest_profiles(&project_dir.join(".config/nextest.toml"))
            })
            .unwrap_or_else(|| RunConfig::default().profiles);
        let mut filter_presets = profile_filter_presets(&profiles);
        filter_presets.extend(ignored_reason_presets(tests));
        RunConfig {
            profiles,
            filter_presets,
        }
    }
}

enum RunProcessOutcome {
    Finished(ExitStatus),
    Stopped,
}

const STOP_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[cfg(unix)]
fn configure_run_command(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
fn configure_run_command(_command: &mut Command) {}

async fn wait_for_stopped_child(
    child: &mut Child,
    tx: &mpsc::Sender<RunEvent>,
) -> Result<ExitStatus> {
    match time::timeout(STOP_GRACE_PERIOD, child.wait()).await {
        Ok(status) => status.context("waiting for stopped nextest"),
        Err(_) => {
            let _ = tx
                .send(RunEvent::RunnerOutput(
                    "Run did not stop promptly; forcing termination".to_owned(),
                ))
                .await;
            if let Err(error) = force_kill_child_process_tree(child) {
                let _ = tx
                    .send(RunEvent::RunnerOutput(format!(
                        "Failed to force stop nextest: {error}"
                    )))
                    .await;
            }
            child
                .wait()
                .await
                .context("waiting for force-stopped nextest")
        }
    }
}

fn terminate_child_process_tree(child: &mut Child) -> io::Result<()> {
    signal_child_process_tree(child, StopSignal::Terminate)
}

fn force_kill_child_process_tree(child: &mut Child) -> io::Result<()> {
    signal_child_process_tree(child, StopSignal::Kill)
}

enum StopSignal {
    Terminate,
    Kill,
}

#[cfg(unix)]
fn signal_child_process_tree(child: &mut Child, signal: StopSignal) -> io::Result<()> {
    let raw_signal = match signal {
        StopSignal::Terminate => libc::SIGTERM,
        StopSignal::Kill => libc::SIGKILL,
    };

    match signal_child_process_group(child, raw_signal) {
        Ok(()) => Ok(()),
        Err(group_error) => child.start_kill().map_err(|child_error| {
            io::Error::new(
                child_error.kind(),
                format!(
                    "process group signal failed: {group_error}; child kill failed: {child_error}"
                ),
            )
        }),
    }
}

#[cfg(not(unix))]
fn signal_child_process_tree(child: &mut Child, _signal: StopSignal) -> io::Result<()> {
    child.start_kill()
}

#[cfg(unix)]
fn signal_child_process_group(child: &Child, signal: libc::c_int) -> io::Result<()> {
    let Some(pid) = child.id() else {
        return Ok(());
    };
    let process_group = -(pid as libc::pid_t);
    let result = unsafe { libc::kill(process_group, signal) };
    if result == 0 {
        return Ok(());
    }

    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
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

fn read_nextest_profiles(path: &Path) -> Option<Vec<NextestProfile>> {
    let text = fs::read_to_string(path).ok()?;
    Some(parse_nextest_profiles(&text))
}

fn parse_nextest_profiles(text: &str) -> Vec<NextestProfile> {
    let mut profiles = Vec::<NextestProfile>::new();
    let mut current_profile = None::<String>;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            current_profile = parse_profile_section(line);
            if let Some(name) = &current_profile {
                upsert_profile(&mut profiles, name.clone(), None);
            }
            continue;
        }

        let Some(profile_name) = &current_profile else {
            continue;
        };
        if let Some(default_filter) = parse_toml_string_value(line, "default-filter") {
            upsert_profile(&mut profiles, profile_name.clone(), Some(default_filter));
        }
    }

    upsert_profile(&mut profiles, "default".to_owned(), None);
    profiles.sort_by(|left, right| {
        profile_sort_key(&left.name)
            .cmp(&profile_sort_key(&right.name))
            .then_with(|| left.name.cmp(&right.name))
    });
    profiles
}

fn parse_profile_section(line: &str) -> Option<String> {
    let body = line.strip_prefix('[')?.strip_suffix(']')?;
    let name = body.strip_prefix("profile.")?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(quoted) = name
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Some(quoted.to_owned());
    }
    if name.contains('.') {
        return None;
    }
    Some(name.to_owned())
}

fn parse_toml_string_value(line: &str, key: &str) -> Option<String> {
    let (raw_key, raw_value) = line.split_once('=')?;
    if raw_key.trim() != key {
        return None;
    }
    let value = raw_value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn upsert_profile(
    profiles: &mut Vec<NextestProfile>,
    name: String,
    default_filter: Option<String>,
) {
    if let Some(profile) = profiles.iter_mut().find(|profile| profile.name == name) {
        if default_filter.is_some() {
            profile.default_filter = default_filter;
        }
        return;
    }
    profiles.push(NextestProfile {
        name,
        default_filter,
    });
}

fn profile_sort_key(name: &str) -> (usize, &str) {
    if name == "default" {
        (0, name)
    } else {
        (1, name)
    }
}

fn profile_filter_presets(profiles: &[NextestProfile]) -> Vec<FilterPreset> {
    profiles
        .iter()
        .filter_map(|profile| {
            let expression = profile.default_filter.as_ref()?;
            Some(FilterPreset::Filterset {
                name: format!("profile {} default-filter", profile.name),
                expression: expression.clone(),
            })
        })
        .collect()
}

fn ignored_reason_presets(tests: &[DiscoveredTest]) -> Vec<FilterPreset> {
    let mut by_reason = BTreeMap::<String, BTreeSet<TestSelector>>::new();
    for test in tests.iter().filter(|test| test.ignored) {
        let Some(reason) = test
            .ignore_reason
            .as_ref()
            .filter(|reason| !reason.is_empty())
        else {
            continue;
        };
        by_reason
            .entry(reason.clone())
            .or_default()
            .insert(TestSelector::from_test(test));
    }

    by_reason
        .into_iter()
        .map(|(reason, tests)| FilterPreset::IgnoredReason {
            reason,
            tests: tests.into_iter().collect(),
        })
        .collect()
}

fn annotate_ignore_reasons(tests: &mut [DiscoveredTest]) {
    for test in tests.iter_mut().filter(|test| test.ignored) {
        let Some(path) = &test.source_path else {
            continue;
        };
        test.ignore_reason = source::ignore_reason_for_test(path, &test.full_name);
    }
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
                ignore_reason: None,
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

async fn consume_success_output_line(
    line: &str,
    collector: &Arc<Mutex<SuccessfulOutputCollector>>,
    tx: &mpsc::Sender<RunEvent>,
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
        let _ = tx.send(event).await;
    }
    consumed
}

async fn flush_success_output(
    collector: &Arc<Mutex<SuccessfulOutputCollector>>,
    tx: &mpsc::Sender<RunEvent>,
) {
    let event = collector
        .lock()
        .ok()
        .and_then(|mut collector| collector.finish_event());
    if let Some(event) = event {
        let _ = tx.send(event).await;
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
