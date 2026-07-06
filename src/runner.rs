use std::time::Duration;

use anyhow::Result;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect},
    config::AppSettings,
    command::{AppCommand, command_for_input},
    config,
    disk_usage,
    editor::EditorConfig,
    git_status,
    input::InputSource,
    nextest::{DiscoveryEvent, NextestClient, RunEvent, RunRequest},
    queue::{self, QueueEvent, QueueSender},
    terminal::AppTerminal,
    theme::Theme,
    ui,
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

    let discovery = start_discovery(client.clone(), queue_tx.clone());
    let git_status = start_git_status(
        client.project_dir(),
        queue_tx.clone(),
    );
    app.begin_disk_usage_scan();
    let disk_usage = start_disk_usage(
        client.project_dir(),
        queue_tx.clone(),
    );
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let result = run_loop(
        terminal,
        app,
        run_on_start,
        RunLoopContext {
            client,
            theme,
            editor,
            cli_open_with,
            queue_tx,
        },
        queue_rx,
    )
    .await;
    discovery.abort();
    git_status.abort();
    disk_usage.abort();
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
) -> Result<()> {
    let mut run_control = None;
    while !app.should_quit {
        let size = terminal.size()?;
        let layout = ui::layout(
            Rect::new(0, 0, size.width, size.height),
            app.settings.tree_width_percent,
        );
        app.prepare_frame(layout.tree.height, layout.output.height);
        terminal.draw(|frame| ui::draw(frame, app, &context.theme))?;
        let Some(event) = queue_rx.recv().await else {
            break;
        };
        handle_queue_event(
            app,
            &mut context,
            &mut run_on_start,
            event,
            &mut run_control,
        );
    }
    Ok(())
}

struct RunLoopContext<'a> {
    client: &'a NextestClient,
    theme: Theme,
    editor: EditorConfig,
    cli_open_with: Option<String>,
    queue_tx: QueueSender,
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
            let command = command_for_input(&input, app.command_context());
            if let Some(key) = input.key_display() {
                app.record_key(key_echo_text(key, &command));
            }
            let effect = app.apply_command(command);
            handle_effect(app, context, effect, run_control);
        }
        QueueEvent::Discovery(event) => {
            if app.apply_discovery_event(event) && *run_on_start {
                *run_on_start = false;
                *run_control = start_run(
                    app,
                    context.client.clone(),
                    RunRequest::default(),
                    context.queue_tx.clone(),
                );
            }
        }
        QueueEvent::DiskUsage(result) => app.apply_disk_usage(result),
        QueueEvent::CargoClean(result) => {
            if app.apply_cargo_clean(result) {
                app.begin_disk_usage_scan();
                start_disk_usage(
                    context.client.project_dir(),
                    context.queue_tx.clone(),
                );
            }
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
    match effect {
        AppEffect::None => {}
        AppEffect::SaveSettings(settings) => {
            if let Err(error) = config::save(settings) {
                app.status = format!("Failed to save settings: {error}");
            } else {
                apply_runtime_settings(context, &app.settings);
            }
        }
        AppEffect::StartDiscovery => {
            start_discovery(context.client.clone(), context.queue_tx.clone());
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
        AppEffect::OpenOutput(request) => match context
            .editor
            .open_text(&request.title, &request.text)
        {
            Ok(path) => {
                app.status = format!("Opened output {}", path.display());
            }
            Err(error) => {
                app.status = format!("Failed to open output: {error}");
            }
        },
        AppEffect::RefreshDiskUsage => {
            start_disk_usage(
                context.client.project_dir(),
                context.queue_tx.clone(),
            );
        }
        AppEffect::RunCargoClean => {
            start_cargo_clean(
                context.client.project_dir(),
                context.queue_tx.clone(),
            );
        }
    }
}

fn apply_runtime_settings(context: &mut RunLoopContext<'_>, settings: &AppSettings) {
    context.editor = EditorConfig::resolve(
        context.cli_open_with.clone(),
        settings.open_with_command.clone(),
    );
    context.theme = Theme::resolve(settings.theme_mode.into(), settings.color_blind_mode);
}

struct RunControl {
    stop_tx: mpsc::UnboundedSender<()>,
}

fn start_discovery(client: NextestClient, tx: QueueSender) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = client
            .discover()
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(QueueEvent::Discovery(DiscoveryEvent::Finished(result)));
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
            if tx.send(QueueEvent::GitStatus(status)).is_err() {
                break;
            }
        }
    })
}

fn start_disk_usage(
    cwd: Option<std::path::PathBuf>,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let result = disk_usage::load(cwd).await;
        let _ = tx.send(QueueEvent::DiskUsage(result));
    })
}

fn start_cargo_clean(
    cwd: Option<std::path::PathBuf>,
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
        let _ = tx.send(QueueEvent::CargoClean(result));
    })
}

fn start_run(
    app: &mut App,
    client: NextestClient,
    request: RunRequest,
    tx: QueueSender,
) -> Option<RunControl> {
    if !app.begin_run(&request) {
        return None;
    }

    let (run_tx, mut run_rx) = mpsc::unbounded_channel::<RunEvent>();
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if let Err(error) = client.run(request, run_tx.clone(), stop_rx).await {
            let _ = run_tx.send(RunEvent::RunnerOutput(format!(
                "nextest failed to start: {error}"
            )));
            let _ = run_tx.send(RunEvent::RunnerFinished { exit_code: None });
        }
    });

    tokio::spawn(async move {
        while let Some(event) = run_rx.recv().await {
            if tx.send(QueueEvent::Run(event)).is_err() {
                break;
            }
        }
    });

    Some(RunControl { stop_tx })
}
