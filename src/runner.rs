use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect},
    command::{AppCommand, CommandContext, InputMode, command_for_input},
    config,
    config::AppSettings,
    disk_usage,
    editor::EditorConfig,
    git_status,
    input::{InputEvent, InputSource},
    nextest::{DiscoveryEvent, NextestClient, RunEvent, RunRequest},
    queue::{self, QueueEvent, QueueSender},
    request::RequestId,
    terminal::AppTerminal,
    theme::Theme,
    ui,
    xtask::{self, XtaskEvent, XtaskRunRequest},
};

pub async fn run(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    run_on_start: bool,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
) -> Result<()> {
    let (queue_tx, queue_rx) = queue::channel();

    let git_status = start_git_status(client.project_dir(), queue_tx.clone());
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let mut context = RunLoopContext {
        client,
        theme,
        editor,
        cli_open_with,
        queue_tx,
        runtime_settings: RuntimeSettings::from_settings(&app.settings),
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
    while !app.should_quit {
        let size = terminal.size()?;
        let layout = ui::layout(
            Rect::new(0, 0, size.width, size.height),
            app.settings.tree_width_percent,
        );
        let xtask_output_page_size =
            ui::xtask_output_page_size(Rect::new(0, 0, size.width, size.height));
        app.prepare_frame(
            layout.tree.height,
            layout.output.height,
            xtask_output_page_size,
        );
        terminal.draw(|frame| ui::draw(frame, app, &context.theme))?;
        let Some(event) = queue_rx.recv().await else {
            break;
        };
        pending_events.push(event);
        drain_pending_events(&mut queue_rx, &mut pending_events);
        handle_queue_events(
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
) {
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
        handle_queue_event(app, context, run_on_start, event, run_control);
        if app.should_quit {
            break;
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
) {
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
        }
        QueueEvent::Discovery(request_id, event) => {
            if app.apply_discovery_event(request_id, event) && *run_on_start {
                *run_on_start = false;
                *run_control = start_run(
                    app,
                    context.client.clone(),
                    RunRequest::default(),
                    context.queue_tx.clone(),
                );
            }
        }
        QueueEvent::DiskUsage(request_id, result) => app.apply_disk_usage(request_id, result),
        QueueEvent::CargoClean(request_id, result) => {
            let effect = app.apply_cargo_clean(request_id, result);
            handle_effect(app, context, effect, run_control);
        }
        QueueEvent::GitStatus(git_status) => app.apply_git_status(git_status),
        QueueEvent::Run(event) => {
            let finished = matches!(
                event,
                RunEvent::RunnerFinished { .. } | RunEvent::RunnerStopped
            );
            app.apply_run_event(event);
            if finished {
                *run_control = None;
            }
        }
        QueueEvent::Xtask(event) => app.apply_xtask_event(event),
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
            *run_control = start_run(
                app,
                context.client.clone(),
                request,
                context.queue_tx.clone(),
            );
        }
        AppEffect::StopRun => {
            if let Some(control) = run_control {
                if control.stop_tx.send(()).is_err() {
                    app.status = "Run already stopped".to_owned();
                }
            } else {
                app.status = "No run in progress".to_owned();
            }
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
            start_disk_usage(
                context.client.project_dir(),
                request_id,
                context.queue_tx.clone(),
            );
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
    stop_tx: mpsc::UnboundedSender<()>,
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
        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;
            let status = git_status::load(cwd.clone()).await;
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
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = disk_usage::load(cwd).await;
        let _ = tx.send(QueueEvent::DiskUsage(request_id, result)).await;
    })
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
    client: NextestClient,
    request: RunRequest,
    tx: QueueSender,
) -> Option<RunControl> {
    tracing::debug!(?request, "starting run request");
    let Some(disk_usage_request_id) = app.begin_run(&request) else {
        tracing::debug!(?request, "run request ignored");
        return None;
    };

    start_disk_usage(client.project_dir(), disk_usage_request_id, tx.clone());

    let (run_tx, mut run_rx) = mpsc::channel::<RunEvent>(queue::APP_EVENT_QUEUE_CAPACITY);
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if let Err(error) = client.run(request, run_tx.clone(), stop_rx).await {
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

    tokio::spawn(async move {
        while let Some(event) = run_rx.recv().await {
            if tx.send(QueueEvent::Run(event)).await.is_err() {
                break;
            }
        }
    });

    Some(RunControl { stop_tx })
}

#[cfg(test)]
mod tests;
