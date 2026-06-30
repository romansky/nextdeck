mod app;
mod command;
mod git_status;
mod input;
mod nextest;
mod output;
mod queue;
mod runner;
mod state;
mod terminal;
mod theme;
mod tree;
mod ui;

use std::{io, path::PathBuf};

use anyhow::Result;
use app::App;
use clap::Parser;
use nextest::NextestClient;
use terminal::TerminalSession;

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
    let run_on_start = cli.run;
    let client = NextestClient::new(cli.manifest_path, cli.current_dir, cli.nextest_args);
    if cli.list_json {
        let tests = client.discover().await?;
        serde_json::to_writer_pretty(io::stdout(), &tests)?;
        println!();
        return Ok(());
    }

    let mut app = App::discovering();
    let mut terminal = TerminalSession::enter()?;
    let result = runner::run(terminal.terminal_mut(), &mut app, &client, run_on_start).await;
    terminal.restore()?;
    result
}
