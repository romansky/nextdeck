use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::App,
    test_events::{TestEventRunLog, TestEventsFocus, TestEventsState},
    theme::Theme,
};

use super::super::{
    geometry::centered_rect,
    primitives::{ModalChrome, draw_modal_shell},
    view_helpers::fit_line_prefix,
};
use super::output::OutputPanel;

pub(in crate::ui) struct TestEventsModal<'a> {
    app: &'a App,
}

struct TestEventsAreas {
    runs: Rect,
    separator: Rect,
    output: Rect,
}

impl<'a> TestEventsModal<'a> {
    pub(in crate::ui) fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(88, 82, frame.area());
        let inner = draw_modal_shell(
            frame,
            theme,
            area,
            ModalChrome {
                title: "Test Events",
                actions: Some(Self::actions(&self.app.test_events)),
            },
        );
        let areas = Self::areas(inner);
        Self::render_runs_panel(
            frame,
            theme,
            areas.runs,
            &self.app.test_events,
            self.app.test_events.focus == TestEventsFocus::Runs,
        );
        frame.render_widget(Clear, areas.separator);
        Self::render_output_panel(
            frame,
            self.app,
            theme,
            areas.output,
            self.app.test_events.focus == TestEventsFocus::Events,
        );
    }

    pub(in crate::ui) fn output_area(area: Rect) -> Rect {
        Self::areas(area).output
    }

    fn areas(area: Rect) -> TestEventsAreas {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Length(1),
                Constraint::Percentage(66),
            ])
            .split(area);
        TestEventsAreas {
            runs: chunks[0],
            separator: chunks[1],
            output: chunks[2],
        }
    }

    fn actions(test_events: &TestEventsState) -> &'static str {
        match test_events.focus {
            TestEventsFocus::Runs => "[esc]close [tab]events",
            TestEventsFocus::Events => "[esc]close [tab]runs [/]search [n/N]match",
        }
    }

    fn render_runs_panel(
        frame: &mut Frame<'_>,
        theme: &Theme,
        area: Rect,
        test_events: &TestEventsState,
        focused: bool,
    ) {
        let block = theme.panel_block("Runs", focused.then_some("[tab]events"), focused);
        let inner = block.inner(area);
        let content_width = inner.width.max(1) as usize;
        let paragraph = Paragraph::new(Self::run_lines(test_events, theme, content_width, focused))
            .style(theme.text())
            .wrap(Wrap { trim: false });
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(paragraph, inner);
    }

    fn run_lines(
        test_events: &TestEventsState,
        theme: &Theme,
        content_width: usize,
        focused: bool,
    ) -> Vec<Line<'static>> {
        if test_events.runs.is_empty() {
            return vec![
                Line::styled("No runs yet.", theme.muted()),
                Line::from(""),
                Line::styled("Run tests from NextDeck to collect events.", theme.text()),
            ];
        }
        test_events
            .runs
            .iter()
            .enumerate()
            .map(|(index, run)| {
                Self::run_line(index, run, test_events, theme, content_width, focused)
            })
            .collect()
    }

    fn run_line(
        index: usize,
        run: &TestEventRunLog,
        test_events: &TestEventsState,
        theme: &Theme,
        content_width: usize,
        focused: bool,
    ) -> Line<'static> {
        let selected = index == test_events.selected_run;
        let marker = if selected { ">" } else { " " };
        let style = if focused && selected {
            theme.selected()
        } else {
            theme.text()
        };
        let label = format!(
            "{marker} {:<8} {:>4} {}",
            run.status,
            run.event_count(),
            run.scope
        );
        Line::styled(fit_line_prefix(&label, content_width), style)
    }

    fn render_output_panel(
        frame: &mut Frame<'_>,
        app: &App,
        theme: &Theme,
        area: Rect,
        focused: bool,
    ) {
        OutputPanel::new(
            &app.test_events.output,
            app.test_events.output_text(),
            "Events",
            focused,
        )
        .render(frame, theme, area);
    }
}
