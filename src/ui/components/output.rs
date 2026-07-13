use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Clear,
};

use crate::{
    output_pane::{OutputPaneState, OutputSearchState, OutputView},
    scroll::ViewportState,
    theme::Theme,
};

use super::super::primitives::scrollable_paragraph;

pub(in crate::ui) struct OutputPanel<'a> {
    state: &'a OutputPaneState,
    source_text: String,
    label: String,
    focused: bool,
}

pub(in crate::ui) struct OutputPanelContent<'a> {
    pub(in crate::ui) status: String,
    pub(in crate::ui) actions: String,
    pub(in crate::ui) lines: Vec<Line<'static>>,
    pub(in crate::ui) viewport: &'a ViewportState,
}

struct PanelChrome<'a> {
    status: &'a str,
    actions: &'a str,
}

impl<'a> OutputPanel<'a> {
    pub(in crate::ui) fn new(
        state: &'a OutputPaneState,
        source_text: String,
        label: impl Into<String>,
        focused: bool,
    ) -> Self {
        Self {
            state,
            source_text,
            label: label.into(),
            focused,
        }
    }

    pub(in crate::ui) fn render(self, frame: &mut Frame<'_>, theme: &Theme, area: Rect) {
        let focused = self.focused;
        let output = self.content(theme);
        draw_output_panel(
            frame,
            theme,
            area,
            PanelChrome {
                status: &output.status,
                actions: &output.actions,
            },
            output.lines,
            focused,
            output.viewport,
        );
    }

    pub(in crate::ui) fn content(&self, theme: &Theme) -> OutputPanelContent<'a> {
        let output_view = self.state.output_view(&self.source_text);
        let search_actions = self.state.search_actions(&self.source_text);
        OutputPanelContent {
            status: self.state.status(&self.label, &output_view.text),
            actions: output_actions(&search_actions),
            lines: output_lines(&self.state.search, theme, &output_view),
            viewport: self.state.viewport(),
        }
    }
}

fn draw_output_panel(
    frame: &mut Frame<'_>,
    theme: &Theme,
    area: Rect,
    chrome: PanelChrome<'_>,
    lines: Vec<Line<'static>>,
    focused: bool,
    viewport: &ViewportState,
) {
    let output = scrollable_paragraph(lines, theme, viewport).block(theme.panel_block(
        chrome.status,
        Some(chrome.actions),
        focused,
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(output, area);
}

pub(in crate::ui) fn output_actions(search_actions: &str) -> String {
    format!("{search_actions} [o]pen-editor")
}

pub(in crate::ui) fn output_lines(
    search: &OutputSearchState,
    theme: &Theme,
    output_view: &OutputView,
) -> Vec<Line<'static>> {
    let lines = output_view
        .text
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let source_line = output_view
                .source_lines
                .get(index)
                .copied()
                .unwrap_or(index);
            highlighted_output_line(
                search,
                theme,
                source_line,
                line,
                output_line_style(theme, line),
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![Line::from("")]
    } else {
        lines
    }
}

fn highlighted_output_line(
    search: &OutputSearchState,
    theme: &Theme,
    source_line: usize,
    line: &str,
    base_style: Style,
) -> Line<'static> {
    let ranges = match search.match_ranges(line) {
        Ok(ranges) if !ranges.is_empty() => ranges,
        _ => return Line::styled(line.to_owned(), base_style),
    };
    let mut spans = Vec::new();
    let mut cursor = 0;
    for (start, end) in ranges {
        if start > cursor {
            spans.push(Span::styled(line[cursor..start].to_owned(), base_style));
        }
        let match_style = if search.current_line == Some(source_line)
            && search
                .current_range
                .is_none_or(|range| range == (start, end))
        {
            theme.active_search_match()
        } else {
            theme.search_match()
        };
        spans.push(Span::styled(line[start..end].to_owned(), match_style));
        cursor = end;
    }
    if cursor < line.len() {
        spans.push(Span::styled(line[cursor..].to_owned(), base_style));
    }
    Line::from(spans)
}

fn output_line_style(theme: &Theme, line: &str) -> Style {
    if line.starts_with("Run passed:") {
        theme.success()
    } else if line.starts_with("Run failed:")
        || line.starts_with("Run command failed:")
        || line.starts_with("nextest: failed")
        || line.starts_with("Stack sampling failed:")
    {
        theme.danger()
    } else if line.starts_with("Run stopped:") {
        theme.warning()
    } else if line.starts_with("@ event error") {
        theme.danger()
    } else if line.starts_with("@ event warn") {
        theme.warning()
    } else if line.starts_with("@ event ") {
        theme.accent()
    } else {
        theme.text()
    }
}
