use std::time::Duration;

use anyhow::Result;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{App, AppEffect},
    command::command_for_input,
    input::InputSource,
    nextest::{NextestClient, RunEvent, RunRequest},
    queue::{self, QueueEvent, QueueSender},
    terminal::AppTerminal,
    ui,
};

pub async fn run(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    run_on_start: bool,
) -> Result<()> {
    let (queue_tx, queue_rx) = queue::channel();

    if run_on_start {
        start_run(app, client.clone(), RunRequest::default(), queue_tx.clone());
    }

    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let result = run_loop(terminal, app, client, queue_tx, queue_rx).await;
    ticker.abort();
    drop(input);
    result
}

async fn run_loop(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    queue_tx: QueueSender,
    mut queue_rx: queue::QueueReceiver,
) -> Result<()> {
    while !app.should_quit {
        let size = terminal.size()?;
        let layout = ui::layout(Rect::new(0, 0, size.width, size.height));
        app.prepare_frame(layout.tree.height, layout.output.height);
        terminal.draw(|frame| ui::draw(frame, app))?;
        let Some(event) = queue_rx.recv().await else {
            break;
        };
        handle_queue_event(app, client, event, queue_tx.clone());
    }
    Ok(())
}

fn handle_queue_event(app: &mut App, client: &NextestClient, event: QueueEvent, tx: QueueSender) {
    match event {
        QueueEvent::Input(input) => {
            if let Some(key) = input.key_display() {
                app.record_key(key);
            }
            let command = command_for_input(&input, app.command_context());
            let effect = app.apply_command(command);
            handle_effect(app, client, effect, tx);
        }
        QueueEvent::Run(event) => app.apply_run_event(event),
        QueueEvent::Tick => app.tick(),
    }
}

fn handle_effect(app: &mut App, client: &NextestClient, effect: AppEffect, tx: QueueSender) {
    match effect {
        AppEffect::None => {}
        AppEffect::StartRun(request) => {
            start_run(app, client.clone(), request, tx);
        }
    }
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
