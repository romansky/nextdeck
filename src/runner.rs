use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect, TestStackSampleRequest},
    command::{AppCommand, CommandContext, InputMode, command_for_input},
    config,
    config::AppSettings,
    diagnostics::{self, ProcessTracker},
    dirty::UiDirty,
    disk_usage,
    editor::EditorConfig,
    git_status,
    input::{InputEvent, InputSource},
    nextest::{DiscoveryEvent, NextestClient, RunEvent, RunRequest},
    queue::{self, QueueEvent, QueueSender},
    request::RequestId,
    terminal::AppTerminal,
    test_events,
    theme::Theme,
    ui,
    xtask::{self, XtaskEvent, XtaskPersistence, XtaskRunRequest},
};

const UI_TICK_INTERVAL: Duration = Duration::from_millis(250);
const GIT_STATUS_INTERVAL: Duration = Duration::from_secs(10);

pub async fn run(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    run_on_start: bool,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
) -> Result<()> {
    let xtask_persistence = XtaskPersistence::resolve(client.project_dir()).await;
    if let Err(error) = xtask_persistence.restore(&mut app.xtasks) {
        tracing::warn!(%error, "failed to restore xtask preferences");
        app.status = format!("Failed to restore xtask preferences: {error}");
    }
    let (queue_tx, queue_rx) = queue::channel();

    let git_status = start_git_status(client.project_dir(), queue_tx.clone());
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), UI_TICK_INTERVAL);
    let mut context = RunLoopContext {
        client,
        theme,
        editor,
        cli_open_with,
        queue_tx,
        runtime_settings: RuntimeSettings::from_settings(&app.settings),
        xtask_persistence,
        disk_usage: None,
    };
    let mut run_control = None;
    for effect in app.startup_effects() {
        handle_effect(app, &mut context, effect, &mut run_control);
    }
    let result = run_loop(terminal, app, run_on_start, context, queue_rx, run_control).await;
    git_status.abort();
    ticker.abort();
    drop(input);
    result
}

async fn run_loop(
    terminal: &mut AppTerminal,
    app: &mut App,
    mut run_on_start: bool,
    mut context: RunLoopContext<'_>,
    mut queue_rx: queue::QueueReceiver,
    mut run_control: Option<RunControl>,
) -> Result<()> {
    let mut pending_events = Vec::new();
    let mut dirty = UiDirty::ALL;
    let result = async {
        while !app.should_quit {
            if dirty.any() {
                draw_frame(terminal, app, &context.theme)?;
                dirty = UiDirty::NONE;
            }
            let Some(event) = queue_rx.recv().await else {
                break;
            };
            pending_events.push(event);
            drain_pending_events(&mut queue_rx, &mut pending_events);
            dirty |= handle_queue_events(
                app,
                &mut context,
                &mut run_on_start,
                &mut pending_events,
                &mut run_control,
            );
            pending_events.clear();
        }
        Ok(())
    }
    .await;

    context.cancel_disk_usage();
    drop(queue_rx);
    if let Some(control) = run_control {
        control.shutdown().await;
    }
    result
}

fn draw_frame(terminal: &mut AppTerminal, app: &mut App, theme: &Theme) -> Result<()> {
    let size = terminal.size()?;
    let area = Rect::new(0, 0, size.width, size.height);
    app.prepare_frame(ui::viewport_metrics(area, app));
    terminal.draw(|frame| ui::draw(frame, app, theme))?;
    Ok(())
}

const MAX_EVENTS_PER_FRAME: usize = 256;

fn drain_pending_events(queue_rx: &mut queue::QueueReceiver, events: &mut Vec<QueueEvent>) {
    while events.len() < MAX_EVENTS_PER_FRAME {
        let Ok(event) = queue_rx.try_recv() else {
            break;
        };
        events.push(event);
    }
}

struct RunLoopContext<'a> {
    client: &'a NextestClient,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
    queue_tx: QueueSender,
    runtime_settings: RuntimeSettings,
    xtask_persistence: XtaskPersistence,
    disk_usage: Option<DiskUsageControl>,
}

impl RunLoopContext<'_> {
    fn refresh_disk_usage(&mut self, request_id: RequestId) {
        self.cancel_disk_usage();
        self.disk_usage = Some(start_disk_usage(
            self.client.project_dir(),
            request_id,
            self.queue_tx.clone(),
        ));
    }

    fn finish_disk_usage(&mut self, request_id: RequestId) {
        if self
            .disk_usage
            .as_ref()
            .is_some_and(|control| control.request_id == request_id)
        {
            self.cancel_disk_usage();
        }
    }

    fn cancel_disk_usage(&mut self) {
        drop(self.disk_usage.take());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeSettings {
    open_with_command: Option<String>,
    theme_mode: config::ThemePreference,
    color_blind_mode: bool,
}

impl RuntimeSettings {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            open_with_command: settings.open_with_command.clone(),
            theme_mode: settings.theme_mode,
            color_blind_mode: settings.color_blind_mode,
        }
    }

    fn theme_changed(&self, next: &Self) -> bool {
        self.theme_mode != next.theme_mode || self.color_blind_mode != next.color_blind_mode
    }

    fn editor_changed(&self, next: &Self) -> bool {
        self.open_with_command != next.open_with_command
    }
}

fn handle_queue_events(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    run_on_start: &mut bool,
    events: &mut [QueueEvent],
    run_control: &mut Option<RunControl>,
) -> UiDirty {
    let mut dirty = UiDirty::NONE;
    if events.len() > 1 {
        tracing::debug!(count = events.len(), "handling pending event batch");
    }
    for index in 0..events.len() {
        let command_context = app.command_context();
        if should_skip_stale_event(events, index, command_context) {
            tracing::debug!(
                index,
                event = ?&events[index],
                context = ?command_context,
                "stale event skipped"
            );
            continue;
        }
        let event = std::mem::replace(&mut events[index], QueueEvent::Tick);
        dirty |= handle_queue_event(app, context, run_on_start, event, run_control);
        if app.should_quit {
            break;
        }
    }
    dirty | flush_xtask_preferences(app, context)
}

fn flush_xtask_preferences(app: &mut App, context: &mut RunLoopContext<'_>) -> UiDirty {
    match context.xtask_persistence.flush(&mut app.xtasks) {
        Ok(_) => UiDirty::NONE,
        Err(error) => {
            tracing::warn!(%error, "failed to persist xtask preferences");
            app.status = format!("Failed to save xtask preferences: {error}");
            UiDirty::STATUS
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LatestOnlyInput {
    TerminalResize,
    TestsPaneWidth,
}

fn should_skip_stale_event(events: &[QueueEvent], index: usize, context: CommandContext) -> bool {
    let Some(class) = latest_only_event(&events[index], context) else {
        return false;
    };
    match class {
        LatestOnlyInput::TerminalResize => events[index + 1..]
            .iter()
            .any(|event| latest_only_event(event, context) == Some(class)),
        LatestOnlyInput::TestsPaneWidth => events[index + 1..]
            .iter()
            .map(|event| latest_only_event(event, context))
            .take_while(Option::is_some)
            .any(|later| later == Some(class)),
    }
}

fn latest_only_event(event: &QueueEvent, context: CommandContext) -> Option<LatestOnlyInput> {
    match event {
        QueueEvent::Input(input) => latest_only_input(input, context),
        QueueEvent::Discovery(_, _)
        | QueueEvent::CargoClean(_, _)
        | QueueEvent::DiskUsage(_, _)
        | QueueEvent::GitStatus(_)
        | QueueEvent::Run(_)
        | QueueEvent::TestStackSample(_)
        | QueueEvent::Xtask(_)
        | QueueEvent::Tick => None,
    }
}

fn latest_only_input(input: &InputEvent, context: CommandContext) -> Option<LatestOnlyInput> {
    match input {
        InputEvent::Terminal(Event::Resize(_, _)) => Some(LatestOnlyInput::TerminalResize),
        InputEvent::Terminal(Event::Key(key))
            if key.kind == KeyEventKind::Press
                && context_accepts_tests_pane_width(context)
                && matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char('[') | KeyCode::Char(']')
                )
                && (key.modifiers.contains(KeyModifiers::SHIFT)
                    || matches!(key.code, KeyCode::Char('[') | KeyCode::Char(']'))) =>
        {
            Some(LatestOnlyInput::TestsPaneWidth)
        }
        InputEvent::Terminal(_) | InputEvent::Error(_) => None,
    }
}

fn context_accepts_tests_pane_width(context: CommandContext) -> bool {
    matches!(context.input, InputMode::Normal(_))
}

fn handle_queue_event(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    run_on_start: &mut bool,
    event: QueueEvent,
    run_control: &mut Option<RunControl>,
) -> UiDirty {
    match event {
        QueueEvent::Input(input) => {
            let command_context = app.command_context();
            let command = command_for_input(&input, command_context);
            tracing::debug!(
                ?input,
                context = ?command_context,
                ?command,
                "input mapped to command"
            );
            if let Some(key) = input.key_display() {
                app.record_key(key_echo_text(key, &command));
            }
            let effect = app.apply_command(command);
            tracing::debug!(?effect, status = %app.status, "command applied");
            handle_effect(app, context, effect, run_control);
            UiDirty::ALL
        }
        QueueEvent::Discovery(request_id, event) => {
            if app.apply_discovery_event(request_id, event) && *run_on_start {
                *run_on_start = false;
                *run_control = start_run(app, context, RunRequest::default());
            }
            UiDirty::ALL
        }
        QueueEvent::DiskUsage(request_id, result) => {
            context.finish_disk_usage(request_id);
            app.apply_disk_usage(request_id, result);
            UiDirty::DETAILS | UiDirty::STATUS
        }
        QueueEvent::CargoClean(request_id, result) => {
            let effect = app.apply_cargo_clean(request_id, result);
            handle_effect(app, context, effect, run_control);
            UiDirty::DETAILS | UiDirty::STATUS | UiDirty::MODAL
        }
        QueueEvent::GitStatus(git_status) => {
            app.apply_git_status(git_status);
            UiDirty::STATUS
        }
        QueueEvent::Run(event) => {
            app.apply_run_event(event);
            UiDirty::TREE | UiDirty::DETAILS | UiDirty::OUTPUT | UiDirty::STATUS | UiDirty::MODAL
        }
        QueueEvent::TestStackSample(result) => {
            app.finish_test_stack_sample(result);
            UiDirty::MODAL | UiDirty::STATUS
        }
        QueueEvent::Xtask(event) => {
            app.apply_xtask_event(event);
            UiDirty::MODAL
        }
        QueueEvent::Tick => app.tick(),
    }
}

fn key_echo_text(key: String, command: &AppCommand) -> String {
    match command.ticker_label() {
        Some(label) => format!("{key} {label}"),
        None => key,
    }
}

fn handle_effect(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    effect: AppEffect,
    run_control: &mut Option<RunControl>,
) {
    tracing::debug!(?effect, "handling app effect");
    match effect {
        AppEffect::None => {}
        AppEffect::SaveSettings(settings) => {
            if let Err(error) = config::save(settings) {
                app.status = format!("Failed to save settings: {error}");
            } else {
                apply_runtime_settings(context, &app.settings);
            }
        }
        AppEffect::StartDiscovery(request_id) => {
            start_discovery(context.client.clone(), request_id, context.queue_tx.clone());
        }
        AppEffect::StartRun(request) => {
            *run_control = start_run(app, context, *request);
        }
        AppEffect::StopRun => {
            if let Some(control) = run_control {
                if !control.request_stop() {
                    app.status = "Run already stopped".to_owned();
                }
            } else {
                app.status = "No run in progress".to_owned();
            }
        }
        AppEffect::SampleTestStacks(request) => {
            let tracker = run_control
                .as_ref()
                .map(|control| control.process_tracker.clone());
            start_test_stack_sample(request, tracker, context.queue_tx.clone());
        }
        AppEffect::OpenSource(location) => match context.editor.open_source(&location) {
            Ok(()) => {
                app.status = format!("Opened source with {}", context.editor.command());
            }
            Err(error) => {
                app.status = format!("Failed to open source: {error}");
            }
        },
        AppEffect::OpenOutput(request) => {
            match context.editor.open_text(&request.title, &request.text) {
                Ok(path) => {
                    app.status = format!("Opened output {}", path.display());
                }
                Err(error) => {
                    app.status = format!("Failed to open output: {error}");
                }
            }
        }
        AppEffect::RefreshDiskUsage(request_id) => {
            context.refresh_disk_usage(request_id);
        }
        AppEffect::RunCargoClean(request_id) => {
            start_cargo_clean(
                context.client.project_dir(),
                request_id,
                context.queue_tx.clone(),
            );
        }
        AppEffect::RefreshXtasks(request_id) => {
            start_xtask_info(
                context.client.project_dir(),
                request_id,
                context.queue_tx.clone(),
            );
        }
        AppEffect::RunXtask(request_id, request) => {
            start_xtask_run(
                context.client.project_dir(),
                request_id,
                request,
                context.queue_tx.clone(),
            );
        }
    }
}

fn apply_runtime_settings(context: &mut RunLoopContext<'_>, settings: &AppSettings) {
    let next = RuntimeSettings::from_settings(settings);
    if context.runtime_settings.editor_changed(&next) {
        context.editor = EditorConfig::resolve(
            context.cli_open_with.clone(),
            next.open_with_command.clone(),
        );
    }
    if context.runtime_settings.theme_changed(&next) {
        tracing::debug!(
            before = ?context.runtime_settings,
            after = ?next,
            "theme runtime settings changed"
        );
        context.theme = Theme::resolve(next.theme_mode.into(), next.color_blind_mode);
    }
    context.runtime_settings = next;
}

struct RunControl {
    stop_tx: Option<mpsc::UnboundedSender<()>>,
    process_tracker: ProcessTracker,
    producer: Option<tokio::task::JoinHandle<()>>,
    forwarder: Option<tokio::task::JoinHandle<()>>,
}

struct DiskUsageControl {
    request_id: RequestId,
    cancellation: disk_usage::DiskScanCancellation,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DiskUsageControl {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.task.abort();
    }
}

impl RunControl {
    fn request_stop(&mut self) -> bool {
        self.stop_tx
            .take()
            .is_some_and(|stop_tx| stop_tx.send(()).is_ok())
    }

    async fn shutdown(mut self) {
        self.request_stop();
        await_run_task(self.producer.take(), "producer").await;
        await_run_task(self.forwarder.take(), "forwarder").await;
    }
}

impl Drop for RunControl {
    fn drop(&mut self) {
        self.request_stop();
    }
}

async fn await_run_task(task: Option<tokio::task::JoinHandle<()>>, name: &str) {
    if let Some(task) = task
        && let Err(error) = task.await
    {
        tracing::warn!(%error, task = name, "run task failed during shutdown");
    }
}

fn start_discovery(
    client: NextestClient,
    request_id: RequestId,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = client
            .discover()
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx
            .send(QueueEvent::Discovery(
                request_id,
                DiscoveryEvent::Finished(result),
            ))
            .await;
    })
}

fn start_git_status(
    cwd: Option<std::path::PathBuf>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(GIT_STATUS_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut previous = None;
        loop {
            ticker.tick().await;
            let status = git_status::load(cwd.clone()).await;
            if previous.as_ref() == Some(&status) {
                continue;
            }
            previous = Some(status.clone());
            if tx.send(QueueEvent::GitStatus(status)).await.is_err() {
                break;
            }
        }
    })
}

fn start_disk_usage(
    cwd: Option<std::path::PathBuf>,
    request_id: RequestId,
    tx: QueueSender,
) -> DiskUsageControl {
    let cancellation = disk_usage::DiskScanCancellation::default();
    let scan_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        let result = match disk_usage::load(cwd, scan_cancellation).await {
            Ok(Some(snapshot)) => Ok(snapshot),
            Ok(None) => return,
            Err(error) => Err(error),
        };
        let _ = tx.send(QueueEvent::DiskUsage(request_id, result)).await;
    });
    DiskUsageControl {
        request_id,
        cancellation,
        task,
    }
}

fn start_cargo_clean(
    cwd: Option<std::path::PathBuf>,
    request_id: RequestId,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut command = tokio::process::Command::new("cargo");
        command.arg("clean");
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let result = match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                if stderr.is_empty() {
                    Err(format!("cargo clean exited with {}", output.status))
                } else {
                    Err(stderr)
                }
            }
            Err(error) => Err(format!("failed to run cargo clean: {error}")),
        };
        let _ = tx.send(QueueEvent::CargoClean(request_id, result)).await;
    })
}

fn start_test_stack_sample(
    request: TestStackSampleRequest,
    process_tracker: Option<ProcessTracker>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    let root_pid = process_tracker.as_ref().and_then(ProcessTracker::root_pid);
    tokio::spawn(async move {
        let TestStackSampleRequest { selector, .. } = request;
        let result = tokio::task::spawn_blocking(move || {
            diagnostics::sample_running_test_stacks(root_pid, &selector)
        })
        .await
        .unwrap_or_else(|error| Err(format!("Stack sampling task failed: {error}")));
        let _ = tx.send(QueueEvent::TestStackSample(result)).await;
    })
}

fn start_xtask_info(
    cwd: Option<std::path::PathBuf>,
    request_id: RequestId,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = xtask::load(cwd).await.map_err(|error| format!("{error:#}"));
        let _ = tx
            .send(QueueEvent::Xtask(XtaskEvent::InfoLoaded {
                request_id,
                result,
            }))
            .await;
    })
}

fn start_xtask_run(
    cwd: Option<std::path::PathBuf>,
    request_id: RequestId,
    request: XtaskRunRequest,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (chunk_tx, mut chunk_rx) = mpsc::channel(queue::APP_EVENT_QUEUE_CAPACITY);
        let output_tx = tx.clone();
        let output_forwarder = tokio::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                if output_tx
                    .send(QueueEvent::Xtask(XtaskEvent::RunOutput {
                        request_id,
                        chunk,
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        let result = xtask::run_streaming(cwd, request, chunk_tx)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = output_forwarder.await;
        let _ = tx
            .send(QueueEvent::Xtask(XtaskEvent::RunFinished {
                request_id,
                result,
            }))
            .await;
    })
}

fn start_run(
    app: &mut App,
    context: &mut RunLoopContext<'_>,
    request: RunRequest,
) -> Option<RunControl> {
    tracing::debug!(?request, "starting run request");
    let Some(disk_usage_request_id) = app.begin_run(&request) else {
        tracing::debug!(?request, "run request ignored");
        return None;
    };

    context.refresh_disk_usage(disk_usage_request_id);
    let client = context.client.clone();
    let tx = context.queue_tx.clone();
    let test_event_run = test_events::create_run();
    app.begin_test_event_run(test_event_run);

    let (run_tx, run_rx) = mpsc::channel::<RunEvent>(queue::APP_EVENT_QUEUE_CAPACITY);
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    let process_tracker = ProcessTracker::default();
    let run_process_tracker = process_tracker.clone();
    let info_output_poll_interval = app.settings.test_output_poll_interval();
    let producer = tokio::spawn(async move {
        if let Err(error) = client
            .run(
                request,
                run_tx.clone(),
                stop_rx,
                true,
                run_process_tracker,
                info_output_poll_interval,
            )
            .await
        {
            let _ = run_tx
                .send(RunEvent::RunnerOutput(format!(
                    "nextest failed to start: {error}"
                )))
                .await;
            let _ = run_tx
                .send(RunEvent::RunnerFinished { exit_code: None })
                .await;
        }
    });

    let forwarder = tokio::spawn(forward_run_events(run_rx, tx));

    Some(RunControl {
        stop_tx: Some(stop_tx),
        process_tracker,
        producer: Some(producer),
        forwarder: Some(forwarder),
    })
}

async fn forward_run_events(mut run_rx: mpsc::Receiver<RunEvent>, tx: QueueSender) {
    let mut terminal_event = None;
    while let Some(event) = run_rx.recv().await {
        if matches!(
            event,
            RunEvent::RunnerFinished { .. } | RunEvent::RunnerStopped
        ) {
            terminal_event = Some(event);
        } else if tx.send(QueueEvent::Run(event)).await.is_err() {
            return;
        }
    }
    if let Some(event) = terminal_event {
        let _ = tx.send(QueueEvent::Run(event)).await;
    }
}

#[cfg(test)]
mod tests;
