use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders},
};

use crate::tree::TestStatus;

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
    pub const fn default() -> Self {
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
