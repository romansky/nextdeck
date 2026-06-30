mod app;
mod command;
mod input;
mod nextest;
mod output;
mod queue;
mod state;
mod terminal;
mod tree;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use anyhow::Result;
use app::{App, AppEffect};
use clap::Parser;
use command::command_for_input;
use input::InputSource;
use nextest::{NextestClient, RunEvent, RunRequest};
use queue::{QueueEvent, QueueSender};
use terminal::{AppTerminal, TerminalSession};
use tokio::sync::mpsc;
use tree::Tree;

#[derive(Debug, Parser)]
#[command(version, about = "Terminal-native UI for cargo-nextest")]
struct Cli {
    #[arg(long)]
    manifest_path: Option<PathBuf>,

    #[arg(long)]
    current_dir: Option<PathBuf>,

    #[arg(long, help = "Run all discovered tests immediately on startup")]
    run: bool,

    #[arg(long, help = "Print discovered tests as JSON and exit")]
    list_json: bool,

    #[arg(
        last = true,
        help = "Additional arguments forwarded to cargo nextest list/run"
    )]
    nextest_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let client = NextestClient::new(cli.manifest_path, cli.current_dir, cli.nextest_args);
    let tests = client.discover().await?;
    if cli.list_json {
        serde_json::to_writer_pretty(io::stdout(), &tests)?;
        println!();
        return Ok(());
    }

    let mut app = App::new(Tree::from_tests(tests));
    let (queue_tx, queue_rx) = queue::channel();

    if cli.run {
        start_run(
            &mut app,
            client.clone(),
            RunRequest::default(),
            queue_tx.clone(),
        );
    }

    let mut terminal = TerminalSession::enter()?;
    let input = InputSource::start(queue_tx.clone());
    let ticker = queue::start_ticker(queue_tx.clone(), Duration::from_millis(250));
    let result = run_app(
        terminal.terminal_mut(),
        &mut app,
        &client,
        queue_tx,
        queue_rx,
    )
    .await;
    ticker.abort();
    drop(input);
    terminal.restore()?;
    result
}

async fn run_app(
    terminal: &mut AppTerminal,
    app: &mut App,
    client: &NextestClient,
    queue_tx: QueueSender,
    mut queue_rx: queue::QueueReceiver,
) -> Result<()> {
    while !app.should_quit {
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
            let command = command_for_input(&input, app.command_context());
            let effect = app.apply_command(command);
            handle_effect(app, client, effect, tx);
        }
        QueueEvent::Run(event) => app.apply_run_event(event),
        QueueEvent::Tick => {}
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
