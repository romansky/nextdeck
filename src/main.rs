mod app;
mod events;
mod nextest;
mod output;
mod state;
mod tree;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nextest::{NextestClient, RunRequest};
use ratatui::{Terminal, backend::CrosstermBackend};
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

    if cli.run {
        app.start_run(client.clone(), RunRequest::default());
    }

    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, &mut app, &client).await;
    restore_terminal(&mut terminal)?;
    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    client: &NextestClient,
) -> Result<()> {
    while !app.should_quit {
        app.drain_run_events();
        terminal.draw(|frame| ui::draw(frame, app))?;
        events::handle_input(app, client)?;
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
