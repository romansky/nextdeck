use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use nextdeck_test_events::{SCHEMA_VERSION, TestEvent};
use tokio::sync::{mpsc, oneshot};

use crate::{
    nextest::RunEvent,
    output::{OUTPUT_TEXT_LIMIT_BYTES, append_bounded_text, bounded_text_with_limit},
    output_pane::OutputPaneState,
};

const MAX_EVENT_RUNS: usize = 20;
const MAX_EVENTS_PER_RUN: usize = 2_000;
const EVENT_RUN_RETENTION_BYTES: usize = OUTPUT_TEXT_LIMIT_BYTES;
const EVENT_DETAIL_LIMIT_BYTES: usize = 64 * 1024;
const EVENT_SUMMARY_LIMIT_BYTES: usize = 4 * 1024;
const EVENTS_TRUNCATED_MARKER: &str = "[... earlier events discarded ...]\n";
const TAIL_INTERVAL: Duration = Duration::from_millis(100);

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestEventRun {
    pub id: String,
    pub dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TestEventsFocus {
    #[default]
    Runs,
    Events,
}

#[derive(Clone, Debug)]
pub struct TestEventsState {
    pub modal_open: bool,
    pub focus: TestEventsFocus,
    pub selected_run: usize,
    pub output: OutputPaneState,
    pub runs: Vec<TestEventRunLog>,
    active_run_id: Option<String>,
    unread: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestEventRunLog {
    pub id: String,
    pub dir: PathBuf,
    pub scope: String,
    pub status: String,
    events: VecDeque<RetainedTestEvent>,
    retained_bytes: usize,
    total_events: usize,
    events_truncated: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct RetainedTestEvent {
    summary: String,
    detail: String,
}

pub struct TestEventTailer {
    stop_tx: oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
struct EventLogTailState {
    files: BTreeMap<PathBuf, EventLogFileTail>,
}

#[derive(Default)]
struct EventLogFileTail {
    offset: u64,
    pending: String,
}

impl Default for TestEventsState {
    fn default() -> Self {
        Self {
            modal_open: false,
            focus: TestEventsFocus::Runs,
            selected_run: 0,
            output: OutputPaneState::default(),
            runs: Vec::new(),
            active_run_id: None,
            unread: false,
        }
    }
}

impl TestEventsState {
    pub fn begin_run(&mut self, run: TestEventRun, scope: String) {
        self.make_room_for_run();
        self.active_run_id = Some(run.id.clone());
        self.runs.push(TestEventRunLog::new(run, scope, "running"));
        self.selected_run = self.runs.len().saturating_sub(1);
        self.output.reset_for_source_change();
    }

    pub fn finish_active_run(&mut self, status: impl Into<String>) {
        let Some(active_run_id) = self.active_run_id.take() else {
            return;
        };
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == active_run_id) {
            run.status = status.into();
        }
    }

    pub fn append_event(&mut self, run_id: &str, event: TestEvent) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == run_id) {
            run.append_event(&event);
        } else {
            self.make_room_for_run();
            let mut run = TestEventRunLog::new(
                TestEventRun {
                    id: run_id.to_owned(),
                    dir: PathBuf::new(),
                },
                "external".to_owned(),
                "unknown",
            );
            run.append_event(&event);
            self.runs.push(run);
            self.selected_run = self.runs.len().saturating_sub(1);
        }
        if !self.modal_open {
            self.unread = true;
        }
    }

    fn make_room_for_run(&mut self) {
        if self.runs.len() < MAX_EVENT_RUNS {
            return;
        }
        let overflow = self.runs.len() + 1 - MAX_EVENT_RUNS;
        self.runs.drain(0..overflow);
        self.selected_run = self.selected_run.saturating_sub(overflow);
    }

    pub fn open(&mut self) {
        self.modal_open = true;
        self.unread = false;
        self.focus = TestEventsFocus::Runs;
        self.selected_run = self.runs.len().saturating_sub(1);
        self.output.reset_for_source_change();
    }

    pub fn close(&mut self) {
        self.modal_open = false;
        self.output.search.close_interaction();
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            TestEventsFocus::Runs => TestEventsFocus::Events,
            TestEventsFocus::Events => TestEventsFocus::Runs,
        };
    }

    pub fn focus_events(&mut self) {
        self.focus = TestEventsFocus::Events;
    }

    pub fn select_next_run(&mut self) {
        if self.runs.is_empty() {
            return;
        }
        self.selected_run = (self.selected_run + 1).min(self.runs.len() - 1);
        self.output.reset_for_source_change();
    }

    pub fn select_previous_run(&mut self) {
        self.selected_run = self.selected_run.saturating_sub(1);
        self.output.reset_for_source_change();
    }

    pub fn selected_run(&self) -> Option<&TestEventRunLog> {
        self.runs.get(self.selected_run)
    }

    pub fn latest_event_label(&self) -> String {
        let Some(event) = self
            .runs
            .iter()
            .rev()
            .find_map(TestEventRunLog::latest_event)
        else {
            return "-".to_owned();
        };
        let suffix = if self.unread { " •" } else { "" };
        format!("{}{}", event.summary, suffix)
    }

    pub fn output_text(&self) -> String {
        let Some(run) = self.selected_run() else {
            return "No test event runs yet.\nRun tests from NextDeck to create an event stream."
                .to_owned();
        };
        if run.events.is_empty() {
            return format!(
                "No events captured for run {}\ndir: {}",
                run.id,
                run.dir.display()
            );
        }
        render_events(run)
    }
}

impl TestEventRunLog {
    fn new(run: TestEventRun, scope: String, status: &str) -> Self {
        Self {
            id: run.id,
            dir: run.dir,
            scope,
            status: status.to_owned(),
            events: VecDeque::new(),
            retained_bytes: 0,
            total_events: 0,
            events_truncated: false,
        }
    }

    fn append_event(&mut self, event: &TestEvent) {
        let event = RetainedTestEvent::new(event);
        self.retained_bytes += event.retained_bytes();
        self.total_events += 1;
        self.events.push_back(event);
        while self.events.len() > MAX_EVENTS_PER_RUN
            || self.retained_bytes > EVENT_RUN_RETENTION_BYTES
        {
            let Some(event) = self.events.pop_front() else {
                break;
            };
            self.retained_bytes -= event.retained_bytes();
            self.events_truncated = true;
        }
    }

    fn latest_event(&self) -> Option<&RetainedTestEvent> {
        self.events.back()
    }

    pub fn event_count(&self) -> usize {
        self.total_events
    }
}

impl RetainedTestEvent {
    fn new(event: &TestEvent) -> Self {
        Self {
            summary: bounded_text_with_limit(event_summary(event), EVENT_SUMMARY_LIMIT_BYTES),
            detail: bounded_text_with_limit(render_event_detail(event), EVENT_DETAIL_LIMIT_BYTES),
        }
    }

    fn retained_bytes(&self) -> usize {
        self.summary.len() + self.detail.len()
    }
}

impl TestEventTailer {
    pub async fn stop(self) {
        let _ = self.stop_tx.send(());
        let _ = self.join.await;
    }
}

pub fn create_run_file() -> Result<TestEventRun> {
    create_run_file_in(&event_root())
}

fn create_run_file_in(root: &Path) -> Result<TestEventRun> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    prune_stale_process_dirs(root);
    let process_dir = process_event_dir(root);
    fs::create_dir_all(&process_dir)
        .with_context(|| format!("create {}", process_dir.display()))?;
    prune_run_dirs(&process_dir, MAX_EVENT_RUNS.saturating_sub(1));
    let id = run_id();
    let dir = process_dir.join(&id);
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(TestEventRun { id, dir })
}

pub fn cleanup_process_files() -> Result<()> {
    cleanup_process_files_in(&event_root())
}

fn cleanup_process_files_in(root: &Path) -> Result<()> {
    let process_dir = process_event_dir(root);
    if process_dir.exists() {
        fs::remove_dir_all(&process_dir)
            .with_context(|| format!("remove {}", process_dir.display()))?;
    }
    Ok(())
}

fn event_root() -> PathBuf {
    std::env::temp_dir().join("nextdeck").join("test-events")
}

fn process_event_dir(root: &Path) -> PathBuf {
    root.join(std::process::id().to_string())
}

fn prune_stale_process_dirs(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() || path == process_event_dir(root) {
            continue;
        }
        let active = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
            .is_some_and(process_is_alive);
        if !active {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn prune_run_dirs(process_dir: &Path, retain: usize) {
    let Ok(entries) = fs::read_dir(process_dir) else {
        return;
    };
    let mut dirs = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    dirs.sort_by_key(|entry| {
        (
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok(),
            entry.file_name(),
        )
    });
    let remove_count = dirs.len().saturating_sub(retain);
    for entry in dirs.into_iter().take(remove_count) {
        let _ = fs::remove_dir_all(entry.path());
    }
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

pub fn start_tailer(run_id: String, dir: PathBuf, tx: mpsc::Sender<RunEvent>) -> TestEventTailer {
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let join = tokio::spawn(async move {
        let mut state = EventLogTailState::default();
        loop {
            forward_new_events(&run_id, &dir, &mut state, false, &tx).await;
            tokio::select! {
                _ = tokio::time::sleep(TAIL_INTERVAL) => {}
                _ = &mut stop_rx => break,
            }
        }
        forward_new_events(&run_id, &dir, &mut state, true, &tx).await;
    });
    TestEventTailer { stop_tx, join }
}

pub fn parse_event_line(line: &str) -> Result<TestEvent, String> {
    let event = serde_json::from_str::<TestEvent>(line).map_err(|error| error.to_string())?;
    if event.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported test event schema version {}, expected {}",
            event.schema_version, SCHEMA_VERSION
        ));
    }
    Ok(event)
}

fn render_events(run: &TestEventRunLog) -> String {
    let mut text = String::new();
    if run.events_truncated {
        append_bounded_text(&mut text, EVENTS_TRUNCATED_MARKER);
    }
    let first_index = run.total_events.saturating_sub(run.events.len()) + 1;
    for (offset, event) in run.events.iter().enumerate() {
        let index = first_index + offset;
        if offset > 0 {
            append_bounded_text(&mut text, "\n");
        }
        append_bounded_text(&mut text, &format!("#{index:<4} {}", event.detail));
    }
    text
}

fn render_event_detail(event: &TestEvent) -> String {
    let mut text = format!(
        "{:>13} {:<5} {:<24} {}\n",
        event.time,
        level_label(event.level),
        event.target.as_deref().unwrap_or("-"),
        event.message
    );
    if !event.fields.is_empty() {
        let fields = serde_json::to_string_pretty(&event.fields)
            .unwrap_or_else(|_| format!("{:?}", event.fields));
        for line in fields.lines() {
            text.push_str("      ");
            text.push_str(line);
            text.push('\n');
        }
    }
    if let Some(source) = &event.source {
        text.push_str(&format!(
            "      at {} {}:{}\n",
            source.module, source.file, source.line
        ));
    }
    text
}

pub fn inline_event_line(event: &TestEvent) -> String {
    event_summary_with_prefix(event, "@ event")
}

fn event_summary(event: &TestEvent) -> String {
    let target = event.target.as_deref().unwrap_or("-");
    if target == "-" {
        format!("{} {}", level_label(event.level), event.message)
    } else {
        format!("{} {}: {}", level_label(event.level), target, event.message)
    }
}

fn event_summary_with_prefix(event: &TestEvent, prefix: &str) -> String {
    let target = event.target.as_deref().unwrap_or("-");
    if target == "-" {
        format!("{prefix} {} {}", level_label(event.level), event.message)
    } else {
        format!(
            "{prefix} {} {}: {}",
            level_label(event.level),
            target,
            event.message
        )
    }
}

async fn forward_new_events(
    run_id: &str,
    dir: &Path,
    state: &mut EventLogTailState,
    final_read: bool,
    tx: &mpsc::Sender<RunEvent>,
) {
    for file in event_log_files(dir) {
        let file_state = state.files.entry(file.clone()).or_default();
        for event in read_new_events(
            &file,
            &mut file_state.offset,
            &mut file_state.pending,
            final_read,
        ) {
            match event {
                Ok(event) => {
                    let _ = tx
                        .send(RunEvent::TestEvent {
                            run_id: run_id.to_owned(),
                            event,
                        })
                        .await;
                }
                Err(error) => {
                    let _ = tx
                        .send(RunEvent::RunnerOutput(format!(
                            "test event parse error: {error}"
                        )))
                        .await;
                }
            }
        }
    }
}

fn event_log_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn read_new_events(
    path: &Path,
    offset: &mut u64,
    pending: &mut String,
    final_read: bool,
) -> Vec<Result<TestEvent, String>> {
    let mut file = match OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut chunk = String::new();
    if file.read_to_string(&mut chunk).is_err() {
        return Vec::new();
    }
    *offset += chunk.len() as u64;
    pending.push_str(&chunk);

    let mut events = Vec::new();
    while let Some(index) = pending.find('\n') {
        let line = pending.drain(..=index).collect::<String>();
        let line = line.trim();
        if !line.is_empty() {
            events.push(parse_event_line(line));
        }
    }
    if final_read && !pending.trim().is_empty() {
        events.push(parse_event_line(pending.trim()));
        pending.clear();
    }
    events
}

fn run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", millis, std::process::id(), counter)
}

pub fn level_label(level: nextdeck_test_events::Level) -> &'static str {
    match level {
        nextdeck_test_events::Level::Trace => "trace",
        nextdeck_test_events::Level::Debug => "debug",
        nextdeck_test_events::Level::Info => "info",
        nextdeck_test_events::Level::Warn => "warn",
        nextdeck_test_events::Level::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextdeck_test_events::Level;
    use serde_json::json;

    #[test]
    fn parse_event_line_rejects_wrong_schema() {
        let error =
            parse_event_line(r#"{"schema_version":2,"time":1,"level":"info","message":"x"}"#)
                .unwrap_err();

        assert!(error.contains("unsupported test event schema version"));
    }

    #[test]
    fn state_tracks_unread_events_until_modal_opens() {
        let mut state = TestEventsState::default();
        state.begin_run(
            TestEventRun {
                id: "run-1".to_owned(),
                dir: PathBuf::from("/tmp/run-1"),
            },
            "workspace".to_owned(),
        );

        state.append_event("run-1", TestEvent::new(Level::Info, "cache hit"));

        assert_eq!(state.latest_event_label(), "info cache hit •");

        state.open();

        assert_eq!(state.latest_event_label(), "info cache hit");
        assert!(state.output_text().contains("cache hit"));
    }

    #[test]
    fn event_log_files_discovers_pid_logs_in_directory() {
        let dir =
            std::env::temp_dir().join(format!("nextdeck-test-events-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create event dir");
        let first = dir.join("123.jsonl");
        let ignored = dir.join("notes.txt");
        std::fs::write(&first, "").expect("write event file");
        std::fs::write(ignored, "").expect("write ignored file");

        assert_eq!(event_log_files(&dir), vec![first]);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn render_events_includes_fields_and_source() {
        let event = TestEvent::new(Level::Info, "cache hit")
            .with_target("artifact-cache")
            .with_fields(std::collections::BTreeMap::from([(
                "key".to_owned(),
                json!("abc"),
            )]))
            .with_source("demo::tests", "src/lib.rs", 42);
        let mut run = TestEventRunLog::new(
            TestEventRun {
                id: "run-1".to_owned(),
                dir: PathBuf::new(),
            },
            "workspace".to_owned(),
            "passed",
        );
        run.append_event(&event);

        let text = render_events(&run);

        assert!(text.contains("artifact-cache"));
        assert!(text.contains("cache hit"));
        assert!(text.contains("\"key\": \"abc\""));
        assert!(text.contains("demo::tests src/lib.rs:42"));
    }

    #[test]
    fn state_bounds_events_by_count_and_preserves_total() {
        let mut state = TestEventsState::default();
        state.begin_run(
            TestEventRun {
                id: "run-1".to_owned(),
                dir: PathBuf::new(),
            },
            "workspace".to_owned(),
        );
        for index in 0..MAX_EVENTS_PER_RUN + 5 {
            state.append_event(
                "run-1",
                TestEvent::new(Level::Info, format!("event-{index}")),
            );
        }

        let run = state.selected_run().expect("run exists");
        assert_eq!(run.event_count(), MAX_EVENTS_PER_RUN + 5);
        assert_eq!(run.events.len(), MAX_EVENTS_PER_RUN);
        assert!(run.events_truncated);
        assert!(state.output_text().contains("earlier events discarded"));
        assert!(state.output_text().contains("event-2004"));
    }

    #[test]
    fn state_bounds_event_details_by_bytes() {
        let mut state = TestEventsState::default();
        state.begin_run(
            TestEventRun {
                id: "run-1".to_owned(),
                dir: PathBuf::new(),
            },
            "workspace".to_owned(),
        );
        for index in 0..20 {
            state.append_event(
                "run-1",
                TestEvent::new(
                    Level::Info,
                    format!("event-{index} {}", "x".repeat(100_000)),
                ),
            );
        }

        let run = state.selected_run().expect("run exists");
        assert!(run.retained_bytes <= EVENT_RUN_RETENTION_BYTES);
        assert!(run.events_truncated);
        assert!(state.output_text().len() <= OUTPUT_TEXT_LIMIT_BYTES);
    }

    #[test]
    fn run_directories_are_scoped_and_pruned_per_process() {
        let root = std::env::temp_dir().join(format!("nextdeck-events-prune-{}", run_id()));
        for _ in 0..MAX_EVENT_RUNS + 3 {
            create_run_file_in(&root).expect("create run directory");
        }

        let process_dir = process_event_dir(&root);
        let run_count = fs::read_dir(&process_dir)
            .expect("read process directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .count();
        assert_eq!(run_count, MAX_EVENT_RUNS);

        cleanup_process_files_in(&root).expect("clean process directory");
        assert!(!process_dir.exists());
        let _ = fs::remove_dir_all(root);
    }
}
