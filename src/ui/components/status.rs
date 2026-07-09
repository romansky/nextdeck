use ratatui::{
    Frame,
    layout::Rect,
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
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(Line::from(Self::spans(self.app, theme))).style(theme.footer()),
            area,
        );
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
            Span::styled(" | run ", theme.footer_label()),
            Span::styled(Self::run_status(app), Self::run_status_style(app, theme)),
            Span::styled(" | storage ", theme.footer_label()),
            Span::styled(storage, Self::storage_status_style(app, theme)),
            Span::styled(" | key ", theme.footer_label()),
            Span::styled(key, theme.footer_value()),
            Span::styled(" | ", theme.footer_label()),
            Span::styled(app.status.as_str(), theme.footer()),
        ]
    }

    pub(in crate::ui) fn run_status(app: &App) -> &'static str {
        app.run_status_label()
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
