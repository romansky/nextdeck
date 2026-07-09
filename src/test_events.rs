use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use nextdeck_test_events::{SCHEMA_VERSION, TestEvent};
use tokio::sync::{mpsc, oneshot};

use crate::{nextest::RunEvent, output_pane::OutputPaneState};

const MAX_EVENT_RUNS: usize = 20;
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
    pub events: Vec<TestEvent>,
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
        if self.runs.len() >= MAX_EVENT_RUNS {
            let overflow = self.runs.len() + 1 - MAX_EVENT_RUNS;
            self.runs.drain(0..overflow);
            self.selected_run = self.selected_run.saturating_sub(overflow);
        }
        self.active_run_id = Some(run.id.clone());
        self.runs.push(TestEventRunLog {
            id: run.id,
            dir: run.dir,
            scope,
            status: "running".to_owned(),
            events: Vec::new(),
        });
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
            run.events.push(event);
        } else {
            self.runs.push(TestEventRunLog {
                id: run_id.to_owned(),
                dir: PathBuf::new(),
                scope: "external".to_owned(),
                status: "unknown".to_owned(),
                events: vec![event],
            });
            self.selected_run = self.runs.len().saturating_sub(1);
        }
        if !self.modal_open {
            self.unread = true;
        }
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
        let Some(event) = self.runs.iter().rev().find_map(|run| run.events.last()) else {
            return "-".to_owned();
        };
        let suffix = if self.unread { " •" } else { "" };
        format!("{}{}", event_summary(event), suffix)
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
        render_events(&run.events)
    }
}

impl TestEventTailer {
    pub async fn stop(self) {
        let _ = self.stop_tx.send(());
        let _ = self.join.await;
    }
}

pub fn create_run_file() -> Result<TestEventRun> {
    let root = std::env::temp_dir().join("nextdeck").join("test-events");
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    let id = run_id();
    let dir = root.join(&id);
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(TestEventRun { id, dir })
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

fn render_events(events: &[TestEvent]) -> String {
    let mut text = String::new();
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&format!(
            "#{:<4} {:>13} {:<5} {:<24} {}\n",
            index + 1,
            event.time,
            level_label(event.level),
            event.target.as_deref().unwrap_or("-"),
            event.message
        ));
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

        let text = render_events(&[event]);

        assert!(text.contains("artifact-cache"));
        assert!(text.contains("cache hit"));
        assert!(text.contains("\"key\": \"abc\""));
        assert!(text.contains("demo::tests src/lib.rs:42"));
    }
}
