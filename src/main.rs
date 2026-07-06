mod app;
mod command;
mod config;
mod disk_usage;
mod editor;
mod git_status;
mod input;
mod nextest;
mod output;
mod output_pane;
mod queue;
mod scroll;
mod settings;
mod source;
mod runner;
mod state;
mod terminal;
mod theme;
mod tree;
mod ui;

use std::{io, path::PathBuf};

use anyhow::Result;
use app::App;
use clap::{Parser, ValueEnum};
use nextest::NextestClient;
use terminal::TerminalSession;
use theme::{Theme, ThemeMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ThemeArg {
    Auto,
    Dark,
    Light,
}

impl From<ThemeArg> for ThemeMode {
    fn from(value: ThemeArg) -> Self {
        match value {
            ThemeArg::Auto => Self::Auto,
            ThemeArg::Dark => Self::Dark,
            ThemeArg::Light => Self::Light,
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "Terminal-native UI for cargo-nextest")]
struct Cli {
    #[arg(long)]
    manifest_path: Option<PathBuf>,

    #[arg(long)]
    current_dir: Option<PathBuf>,

    #[arg(long, help = "Run all discovered tests immediately on startup")]
    run: bool,

    #[arg(long, value_enum, help = "Theme mode to use")]
    theme: Option<ThemeArg>,

    #[arg(
        long,
        help = "Editor command for opening sources/output. Also reads CARGO_TEST_TUI_EDITOR, VISUAL, EDITOR"
    )]
    editor: Option<String>,

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
    let settings = config::load();
    let editor = editor::EditorConfig::resolve(cli.editor.clone(), settings.editor_command.clone());
    let client = NextestClient::new(cli.manifest_path, cli.current_dir, cli.nextest_args);
    if cli.list_json {
        let tests = client.discover().await?;
        serde_json::to_writer_pretty(io::stdout(), &tests)?;
        println!();
        return Ok(());
    }

    let mut app = App::discovering(settings);
    let theme_mode = cli
        .theme
        .map(ThemeMode::from)
        .unwrap_or_else(|| app.settings.theme_mode.into());
    let theme = Theme::resolve(theme_mode, app.settings.color_blind_mode);
    let mut terminal = TerminalSession::enter()?;
    let result = runner::run(
        terminal.terminal_mut(),
        &mut app,
        &client,
        run_on_start,
        theme,
        editor,
        cli.editor,
    )
    .await;
    terminal.restore()?;
    result
}
