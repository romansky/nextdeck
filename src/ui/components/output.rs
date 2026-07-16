use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

#[cfg(test)]
use crate::output_pane::OutputView;
use crate::{
    output_pane::{OutputPaneState, OutputSearchState},
    theme::Theme,
};

pub(in crate::ui) struct OutputPanel<'a> {
    state: &'a OutputPaneState,
    source_text: String,
    label: String,
    focused: bool,
}

pub(in crate::ui) struct OutputPanelContent {
    pub(in crate::ui) status: String,
    pub(in crate::ui) actions: String,
    pub(in crate::ui) lines: Vec<Line<'static>>,
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
        );
    }

    pub(in crate::ui) fn content(&self, theme: &Theme) -> OutputPanelContent {
        let search_actions = self.state.search_actions(&self.source_text);
        OutputPanelContent {
            status: self.state.status(&self.label),
            actions: output_actions(&search_actions),
            lines: output_layout_lines(self.state, theme),
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
) {
    let block = theme.panel_block(chrome.status, Some(chrome.actions), focused);
    let inner = block.inner(area);
    let output = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .style(theme.text());
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(output, inner);
}

pub(in crate::ui) fn output_actions(search_actions: &str) -> String {
    format!("{search_actions} [o]pen-editor")
}

fn output_layout_lines(state: &OutputPaneState, theme: &Theme) -> Vec<Line<'static>> {
    let layout = state.layout();
    let start = state.scroll();
    let end = start
        .saturating_add(state.page_size())
        .min(layout.row_count());
    (start..end)
        .filter_map(|index| layout.row(index))
        .map(|row| {
            let logical_line = layout.logical_line_text(row);
            highlighted_output_row(
                &state.search,
                theme,
                row.source_line(),
                logical_line,
                row.byte_range(),
            )
        })
        .collect()
}

#[cfg(test)]
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

fn highlighted_output_row(
    search: &OutputSearchState,
    theme: &Theme,
    source_line: Option<usize>,
    line: &str,
    row_range: std::ops::Range<usize>,
) -> Line<'static> {
    let base_style = output_line_style(theme, line);
    let ranges = search.match_ranges(line).unwrap_or_default();
    let mut boundaries = vec![row_range.start, row_range.end];
    for (start, end) in &ranges {
        if *start < row_range.end && row_range.start < *end {
            boundaries.push((*start).max(row_range.start));
            boundaries.push((*end).min(row_range.end));
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    if boundaries.len() < 2 {
        return Line::styled(String::new(), base_style);
    }
    let spans = boundaries
        .windows(2)
        .filter(|boundary| boundary[0] < boundary[1])
        .map(|boundary| {
            let start = boundary[0];
            let end = boundary[1];
            let matching_range = ranges
                .iter()
                .copied()
                .find(|(match_start, match_end)| *match_start <= start && end <= *match_end);
            let style = match matching_range {
                Some(range)
                    if source_line == search.current_line
                        && search.current_range.is_none_or(|current| current == range) =>
                {
                    theme.active_search_match()
                }
                Some(_) => theme.search_match(),
                None => base_style,
            };
            Span::styled(line[start..end].to_owned(), style)
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

#[cfg(test)]
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
        || line.starts_with("nextest:")
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
