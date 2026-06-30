use std::time::Duration;

use anyhow::Result;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect},
    command::{AppCommand, command_for_input},
    config,
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
) -> Result<()> {
    let (queue_tx, queue_rx) = queue::channel();

    let discovery = start_discovery(client.clone(), queue_tx.clone());
    let git_status = start_git_status(client.current_dir().map(ToOwned::to_owned), queue_tx.clone());
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let result = run_loop(terminal, app, client, run_on_start, theme, queue_tx, queue_rx).await;
    discovery.abort();
    git_status.abort();
    ticker.abort();
    drop(input);
    result
}

async fn run_loop(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    mut run_on_start: bool,
    theme: Theme,
    queue_tx: QueueSender,
    mut queue_rx: queue::QueueReceiver,
) -> Result<()> {
    while !app.should_quit {
        let size = terminal.size()?;
        let layout = ui::layout(
            Rect::new(0, 0, size.width, size.height),
            app.settings.tree_width_percent,
        );
        app.prepare_frame(layout.tree.height, layout.output.height);
        terminal.draw(|frame| ui::draw(frame, app, &theme))?;
        let Some(event) = queue_rx.recv().await else {
            break;
        };
        handle_queue_event(app, client, &mut run_on_start, event, queue_tx.clone());
    }
    Ok(())
}

fn handle_queue_event(
    app: &mut App,
    client: &NextestClient,
    run_on_start: &mut bool,
    event: QueueEvent,
    tx: QueueSender,
) {
    match event {
        QueueEvent::Input(input) => {
            let command = command_for_input(&input, app.command_context());
            if let Some(key) = input.key_display() {
                app.record_key(key_echo_text(key, &command));
            }
            let effect = app.apply_command(command);
            handle_effect(app, client, effect, tx);
        }
        QueueEvent::Discovery(event) => {
            if app.apply_discovery_event(event) && *run_on_start {
                *run_on_start = false;
                start_run(app, client.clone(), RunRequest::default(), tx);
            }
        }
        QueueEvent::GitStatus(git_status) => app.apply_git_status(git_status),
        QueueEvent::Run(event) => app.apply_run_event(event),
        QueueEvent::Tick => app.tick(),
    }
}

fn key_echo_text(key: String, command: &AppCommand) -> String {
    match command.ticker_label() {
        Some(label) => format!("{key} {label}"),
        None => key,
    }
}

fn handle_effect(app: &mut App, client: &NextestClient, effect: AppEffect, tx: QueueSender) {
    match effect {
        AppEffect::None => {}
        AppEffect::SaveSettings(settings) => {
            if let Err(error) = config::save(settings) {
                app.status = format!("Failed to save settings: {error}");
            }
        }
        AppEffect::StartDiscovery => {
            start_discovery(client.clone(), tx);
        }
        AppEffect::StartRun(request) => {
            start_run(app, client.clone(), request, tx);
        }
    }
}

fn start_discovery(
    client: NextestClient,
    tx: QueueSender,
) -> tokio::task::JoinHandle<()> {
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

fn start_run(app: &mut App, client: NextestClient, request: RunRequest, tx: QueueSender) {
    if !app.begin_run(&request) {
        return;
    }

    let (run_tx, mut run_rx) = mpsc::unbounded_channel::<RunEvent>();
    tokio::spawn(async move {
        if let Err(error) = client.run(request, run_tx.clone()).await {
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
}
