use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::{app::App, theme::Theme};

use super::super::view_helpers::{
    storage_status, storage_status_style as storage_status_style_for,
};

pub(in crate::ui) struct StatusBar<'a> {
    app: &'a App,
}

impl<'a> StatusBar<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(Line::from(Self::action_spans(self.app, theme, area.width)))
                .style(theme.footer()),
            rows[0],
        );
        frame.render_widget(
            Paragraph::new(Line::from(Self::spans(self.app, theme))).style(theme.footer()),
            rows[1],
        );
    }

    pub(in crate::ui) fn action_spans(app: &App, theme: &Theme, width: u16) -> Vec<Span<'static>> {
        let normal = app.command_context().normal_focus().is_some();
        let actions = if width >= 110 {
            vec![
                ("[Tab]focus", normal),
                ("[Shift+Left/[]narrow", normal),
                ("[Shift+Right/]]widen", normal),
                ("[X]tasks", normal),
                ("[E]vents", normal),
                ("[,]settings", normal),
                ("[D]isk-cleanup", normal),
                ("[Q]uit", normal),
            ]
        } else if width >= 78 {
            vec![
                ("[Tab]pane", normal),
                ("[⇧←/[]-", normal),
                ("[⇧→/]]+", normal),
                ("[X]tasks", normal),
                ("[E]vents", normal),
                ("[,]prefs", normal),
                ("[D]cleanup", normal),
                ("[Q]uit", normal),
            ]
        } else if width >= 42 {
            vec![
                ("[Tab]pane", normal),
                ("[Q]uit", normal),
                ("[⇧←/[]-", normal),
                ("[⇧→/]]+", normal),
                ("[X]tasks", normal),
            ]
        } else {
            vec![
                ("[Q]uit", normal),
                ("[Tab]pane", normal),
                ("[X]tasks", normal),
            ]
        };

        let mut spans = Vec::new();
        let mut used = 0;
        for (text, active) in actions {
            let separator = usize::from(!spans.is_empty());
            let next_width = separator + text.chars().count();
            if used + next_width > usize::from(width) {
                continue;
            }
            if separator == 1 {
                spans.push(Span::styled(" ", theme.muted().bg(theme.footer_bg)));
            }
            spans.push(Span::styled(
                text,
                if active {
                    theme.footer_label()
                } else {
                    theme.muted().bg(theme.footer_bg)
                },
            ));
            used += next_width;
        }
        spans
    }

    pub(in crate::ui) fn spans<'b>(app: &'b App, theme: &'b Theme) -> Vec<Span<'b>> {
        let key = app
            .key_echo
            .as_ref()
            .map(|echo| echo.text.as_str())
            .unwrap_or("-");
        let storage = storage_status(app);
        vec![
            Span::styled(" branch ", theme.footer_label()),
            Span::styled(app.git_status.branch.as_str(), theme.footer_value()),
            Span::styled(" | unstaged ", theme.footer_label()),
            Span::styled(
                app.git_status.unstaged.added.to_string(),
                theme.footer_dirty(true),
            ),
            Span::styled(":", theme.footer_label()),
            Span::styled(
                app.git_status.unstaged.deleted.to_string(),
                theme.footer_dirty(false),
            ),
            Span::styled(" | staged ", theme.footer_label()),
            Span::styled(
                app.git_status.staged.added.to_string(),
                theme.footer_dirty(true),
            ),
            Span::styled(":", theme.footer_label()),
            Span::styled(
                app.git_status.staged.deleted.to_string(),
                theme.footer_dirty(false),
            ),
            Span::styled(" | tests: ", theme.footer_label()),
            Span::styled(Self::run_status(app), Self::run_status_style(app, theme)),
            Span::styled(
                if app.running { " [Ctrl+C]stop" } else { "" },
                theme.footer_label(),
            ),
            Span::styled(" | storage ", theme.footer_label()),
            Span::styled(storage, Self::storage_status_style(app, theme)),
            Span::styled(" | key ", theme.footer_label()),
            Span::styled(key, theme.footer_value()),
            Span::styled(" | ", theme.footer_label()),
            Span::styled(app.status.as_str(), theme.footer()),
        ]
    }

    pub(in crate::ui) fn run_status(app: &App) -> &'static str {
        match app.run.phase {
            crate::app::RunPhase::NotRunning => "idle",
            crate::app::RunPhase::Building => "building",
            crate::app::RunPhase::RunningTests => "running",
        }
    }

    fn run_status_style(app: &App, theme: &Theme) -> Style {
        match app.run.phase {
            crate::app::RunPhase::Building | crate::app::RunPhase::RunningTests => theme.accent(),
            crate::app::RunPhase::NotRunning => theme.muted(),
        }
        .bg(theme.footer_bg)
    }

    fn storage_status_style(app: &App, theme: &Theme) -> Style {
        storage_status_style_for(app, theme).bg(theme.footer_bg)
    }
}
