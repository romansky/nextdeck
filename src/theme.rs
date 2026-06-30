use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders},
};
use terminal_colorsaurus::{QueryOptions, ThemeMode as TerminalThemeMode};

use crate::tree::TestStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeMode {
    Auto,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub border: Color,
    pub focused_border: Color,
    pub footer_bg: Color,
    pub footer_fg: Color,
    pub success: Color,
    pub danger: Color,
    pub warning: Color,
}

impl Theme {
    pub fn resolve(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Auto => Self::auto(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    pub fn auto() -> Self {
        let mut options = QueryOptions::default();
        options.timeout = std::time::Duration::from_millis(200);
        match terminal_colorsaurus::theme_mode(options) {
            Ok(TerminalThemeMode::Light) => Self::light(),
            Ok(TerminalThemeMode::Dark) | Err(_) => Self::dark(),
        }
    }

    pub const fn dark() -> Self {
        Self {
            text: Color::Gray,
            muted: Color::DarkGray,
            accent: Color::LightCyan,
            selected_bg: Color::Blue,
            selected_fg: Color::White,
            border: Color::DarkGray,
            focused_border: Color::LightCyan,
            footer_bg: Color::Blue,
            footer_fg: Color::White,
            success: Color::Green,
            danger: Color::Red,
            warning: Color::Yellow,
        }
    }

    pub const fn light() -> Self {
        Self {
            text: Color::Black,
            muted: Color::Gray,
            accent: Color::Blue,
            selected_bg: Color::Blue,
            selected_fg: Color::White,
            border: Color::Gray,
            focused_border: Color::Blue,
            footer_bg: Color::Reset,
            footer_fg: Color::Black,
            success: Color::Green,
            danger: Color::Red,
            warning: Color::Yellow,
        }
    }

    pub fn panel_block<'a>(&self, title: &'a str, focused: bool) -> Block<'a> {
        Block::default()
            .title(Line::styled(format!(" {title} "), self.title(focused)))
            .borders(Borders::ALL)
            .border_style(self.border(focused))
    }

    pub fn modal_block<'a>(&self, title: &'a str) -> Block<'a> {
        Block::default()
            .title(Line::styled(format!(" {title} "), self.title(true)))
            .borders(Borders::ALL)
            .border_style(self.border(true))
    }

    pub fn border(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.focused_border)
        } else {
            Style::default().fg(self.border)
        }
    }

    pub fn title(&self, focused: bool) -> Style {
        if focused {
            Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.muted)
        }
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn danger(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.selected_fg)
            .bg(self.selected_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn footer(&self) -> Style {
        Style::default().fg(self.footer_fg).bg(self.footer_bg)
    }

    pub fn footer_label(&self) -> Style {
        Style::default().fg(Color::LightBlue).bg(self.footer_bg)
    }

    pub fn footer_value(&self) -> Style {
        Style::default()
            .fg(self.footer_fg)
            .bg(self.footer_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn footer_dirty(&self, added: bool) -> Style {
        let fg = if added { self.success } else { self.danger };
        Style::default().fg(fg).bg(self.footer_bg)
    }

    pub fn status(&self, status: TestStatus, selected: bool) -> Style {
        let color = match status {
            TestStatus::Pending => self.muted,
            TestStatus::Running => self.accent,
            TestStatus::Passed => self.success,
            TestStatus::Failed => self.danger,
            TestStatus::Ignored => self.warning,
            TestStatus::Skipped => Color::Magenta,
        };

        if selected {
            Style::default()
                .fg(self.selected_fg)
                .bg(self.selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_modes_select_expected_palettes() {
        assert_eq!(Theme::resolve(ThemeMode::Dark).text, Color::Gray);
        assert_eq!(Theme::resolve(ThemeMode::Light).text, Color::Black);
    }

    #[test]
    fn light_palette_uses_terminal_background_for_footer() {
        assert_eq!(Theme::light().footer_bg, Color::Reset);
    }
}
