use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use terminal_colorsaurus::{QueryOptions, ThemeMode as TerminalThemeMode};

use crate::{config::ThemePreference, tree::TestStatus};

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
        if color_blind_mode {
            theme.color_blind()
        } else {
            theme
        }
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
            text: Color::Rgb(205, 214, 244),
            muted: Color::Rgb(127, 132, 156),
            accent: Color::Rgb(137, 180, 250),
            selected_bg: Color::Rgb(69, 71, 90),
            selected_fg: Color::Rgb(205, 214, 244),
            border: Color::Rgb(49, 50, 68),
            focused_border: Color::Rgb(137, 180, 250),
            footer_bg: Color::Rgb(24, 24, 37),
            footer_fg: Color::Rgb(205, 214, 244),
            success: Color::Rgb(166, 227, 161),
            danger: Color::Rgb(243, 139, 168),
            warning: Color::Rgb(249, 226, 175),
        }
    }

    pub const fn light() -> Self {
        Self {
            text: Color::Rgb(76, 79, 105),
            muted: Color::Rgb(140, 143, 161),
            accent: Color::Rgb(30, 102, 245),
            selected_bg: Color::Rgb(204, 208, 218),
            selected_fg: Color::Rgb(76, 79, 105),
            border: Color::Rgb(188, 192, 204),
            focused_border: Color::Rgb(30, 102, 245),
            footer_bg: Color::Rgb(230, 233, 239),
            footer_fg: Color::Rgb(76, 79, 105),
            success: Color::Rgb(64, 160, 43),
            danger: Color::Rgb(210, 15, 57),
            warning: Color::Rgb(223, 142, 29),
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
        Line::from(vec![
            Span::styled(" ".to_owned(), title_style),
            Span::styled(status.to_owned(), title_style),
            Span::styled(" ".to_owned(), title_style),
        ])
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
            .fg(Color::Black)
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
        Style::default().fg(self.accent).bg(self.footer_bg)
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
