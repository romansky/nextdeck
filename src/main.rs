mod app;
mod command;
mod config;
mod custom_run;
mod diagnostics;
mod disk_usage;
mod editor;
mod git_status;
mod input;
mod input_field;
mod nextest;
mod output;
mod output_pane;
mod parameter_list;
mod queue;
mod request;
mod runner;
mod scroll;
mod settings;
mod source;
mod state;
mod symbols;
mod terminal;
mod test_events;
mod theme;
mod tree;
mod ui;
mod xtask;

use std::{
    fs::OpenOptions,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use app::App;
use clap::{Parser, ValueEnum};
use nextest::NextestClient;
use terminal::TerminalSession;
use theme::{Theme, ThemeMode};
use tracing_subscriber::EnvFilter;

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

    #[arg(
        long,
        help = "Write diagnostic logs to ~/.nextdeck/debug.log for later inspection"
    )]
    debug: bool,

    #[arg(long, value_enum, help = "Theme mode to use")]
    theme: Option<ThemeArg>,

    #[arg(
        long = "open-with",
        alias = "editor",
        help = "Command for opening sources/output. Also reads NEXTDECK_EDITOR, VISUAL, EDITOR"
    )]
    open_with: Option<String>,

    #[arg(long, help = "Print discovered tests as JSON and exit")]
    list_json: bool,

    #[arg(long, help = "Print discovered xtasks as JSON and exit")]
    list_xtasks_json: bool,

    #[arg(
        last = true,
        help = "Additional arguments forwarded to cargo nextest list/run"
    )]
    nextest_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let debug_log_path = init_tracing(cli.debug)?;
    let _debug_log_exit_notice = DebugLogExitNotice::new(debug_log_path.clone());
    if let Some(path) = debug_log_path.as_ref() {
        tracing::debug!(path = %path.display(), "debug logging enabled");
    }

    let run_on_start = cli.run;
    let settings = config::load();
    let editor =
        editor::EditorConfig::resolve(cli.open_with.clone(), settings.open_with_command.clone());
    let client = NextestClient::new(cli.manifest_path, cli.current_dir, cli.nextest_args);
    if cli.list_json {
        let tests = client.discover().await?.tests;
        serde_json::to_writer_pretty(io::stdout(), &tests)?;
        println!();
        return Ok(());
    }
    if cli.list_xtasks_json {
        let manifest = xtask::load(client.project_dir()).await?;
        serde_json::to_writer_pretty(io::stdout(), &manifest)?;
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
        cli.open_with,
    )
    .await;
    terminal.restore()?;
    result
}

fn init_tracing(debug: bool) -> Result<Option<PathBuf>> {
    if debug {
        let path = config::debug_log_path()
            .context("debug logging requires HOME so ~/.nextdeck/debug.log can be created")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        tracing_subscriber::fmt()
            .with_env_filter(debug_env_filter())
            .with_writer(Mutex::new(file))
            .with_ansi(false)
            .init();
        Ok(Some(path))
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
        Ok(None)
    }
}

fn debug_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nextdeck=debug"))
}

struct DebugLogExitNotice {
    path: Option<PathBuf>,
}

impl DebugLogExitNotice {
    fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }
}

impl Drop for DebugLogExitNotice {
    fn drop(&mut self) {
        if let Some(path) = self.path.as_ref() {
            let _ = writeln!(io::stderr(), "{}", debug_log_exit_message(path));
        }
    }
}

fn debug_log_exit_message(path: &Path) -> String {
    format!(
        "nextdeck debug log: {}\nbefore sharing, scrub the log for PII or other sensitive data",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_log_exit_message_includes_path() {
        assert_eq!(
            debug_log_exit_message(Path::new("/tmp/nextdeck-debug.log")),
            "nextdeck debug log: /tmp/nextdeck-debug.log\nbefore sharing, scrub the log for PII or other sensitive data"
        );
    }
}
