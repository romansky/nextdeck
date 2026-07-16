use std::io::{self, Stdout};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalSession {
    terminal: AppTerminal,
    restored: bool,
}

impl TerminalSession {
    pub fn enter() -> Result<Self> {
        enable_raw_mode().context("enabling terminal raw mode")?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
            let error = anyhow::Error::new(error).context("entering alternate terminal screen");
            return match rollback_terminal_setup(false) {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(error.context(format!(
                    "terminal setup rollback failed: {rollback_error:#}"
                ))),
            };
        }

        let terminal = match Terminal::new(CrosstermBackend::new(io::stdout())) {
            Ok(terminal) => terminal,
            Err(error) => {
                let error = anyhow::Error::new(error).context("creating terminal backend");
                return match rollback_terminal_setup(true) {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(error.context(format!(
                        "terminal setup rollback failed: {rollback_error:#}"
                    ))),
                };
            }
        };

        Ok(Self {
            terminal,
            restored: false,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut AppTerminal {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }

        let mut errors = Vec::new();
        if let Err(error) = disable_raw_mode() {
            errors.push(format!("disabling terminal raw mode: {error}"));
        }
        if let Err(error) = execute!(self.terminal.backend_mut(), LeaveAlternateScreen) {
            errors.push(format!("leaving alternate terminal screen: {error}"));
        }
        if let Err(error) = self.terminal.show_cursor() {
            errors.push(format!("showing terminal cursor: {error}"));
        }
        if errors.is_empty() {
            self.restored = true;
            Ok(())
        } else {
            Err(anyhow!(errors.join("; ")))
        }
    }
}

fn rollback_terminal_setup(leave_alternate_screen: bool) -> Result<()> {
    let mut errors = Vec::new();
    if leave_alternate_screen && let Err(error) = execute!(io::stdout(), LeaveAlternateScreen) {
        errors.push(format!("leaving alternate terminal screen: {error}"));
    }
    if let Err(error) = disable_raw_mode() {
        errors.push(format!("disabling terminal raw mode: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errors.join("; ")))
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            tracing::error!("failed to restore terminal: {error:#}");
        }
    }
}
