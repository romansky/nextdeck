use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use terminal_colorsaurus::{QueryOptions, ThemeMode as TerminalThemeMode};

use crate::{
    config::ThemePreference,
    symbols::{DISABLED, ENABLED},
    tree::TestStatus,
};

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
    pub fn resolve(mode: ThemeMode, color_blind_mode: bool) -> Self {
        let theme = match mode {
            ThemeMode::Auto => Self::auto(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        };
        if color_blind_mode { theme.color_blind() } else { theme }
    }

    fn color_blind(mut self) -> Self {
        self.success = Color::Cyan;
        self.danger = Color::Magenta;
        self.warning = Color::Yellow;
        self.selected_bg = Color::DarkGray;
        self.focused_border = self.accent;
        self
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

    pub fn panel_block<'a>(
        &self,
        status: &'a str,
        actions: Option<&'a str>,
        focused: bool,
    ) -> Block<'a> {
        let block = Block::default()
            .title(self.panel_title(status, focused))
            .borders(Borders::ALL)
            .border_style(self.border(focused));
        if let Some(actions) = actions {
            block.title_bottom(Line::styled(
                format!(" {actions} "),
                self.panel_actions(focused),
            ))
        } else {
            block
        }
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

    fn panel_actions(&self, focused: bool) -> Style {
        self.title(focused)
    }

    fn panel_title<'a>(&self, status: &str, focused: bool) -> Line<'a> {
        let title_style = self.title(focused);
        let mut spans = vec![Span::styled(" ".to_owned(), title_style)];
        let mut normal = String::new();
        for char in status.chars() {
            let style = match char {
                ENABLED => Some(self.success().add_modifier(Modifier::BOLD)),
                DISABLED => Some(self.danger().add_modifier(Modifier::BOLD)),
                _ => None,
            };
            let Some(style) = style else {
                normal.push(char);
                continue;
            };
            if !normal.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut normal), title_style));
            }
            spans.push(Span::styled(char.to_string(), style));
        }
        if !normal.is_empty() {
            spans.push(Span::styled(normal, title_style));
        }
        spans.push(Span::styled(" ".to_owned(), title_style));
        Line::from(spans)
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

    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn danger(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn search_match(&self) -> Style {
        Style::default()
            .fg(self.selected_fg)
            .bg(self.warning)
            .add_modifier(Modifier::BOLD)
    }

    pub fn active_search_match(&self) -> Style {
        Style::default()
            .fg(self.selected_fg)
            .bg(self.selected_bg)
            .add_modifier(Modifier::BOLD)
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

impl From<ThemePreference> for ThemeMode {
    fn from(value: ThemePreference) -> Self {
        match value {
            ThemePreference::Auto => Self::Auto,
            ThemePreference::Dark => Self::Dark,
            ThemePreference::Light => Self::Light,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests;
