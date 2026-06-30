use std::io::{self, Stdout};

use anyhow::{Context, Result};
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
            let _ = disable_raw_mode();
            return Err(error).context("entering alternate terminal screen");
        }

        let terminal = match Terminal::new(CrosstermBackend::new(io::stdout())) {
            Ok(terminal) => terminal,
            Err(error) => {
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(error).context("creating terminal backend");
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

        disable_raw_mode().context("disabling terminal raw mode")?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)
            .context("leaving alternate terminal screen")?;
        self.terminal
            .show_cursor()
            .context("showing terminal cursor")?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            tracing::error!("failed to restore terminal: {error:#}");
        }
    }
}
